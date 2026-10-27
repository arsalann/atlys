import AtlysCore
import SwiftUI

/// Every record type in the vault with counts. Browse only: record contents
/// can't be copied or shared out of the app.
struct DataBrowserView: View {
    let model: AppModel
    @State private var types: [RecordTypeSummary] = []
    @State private var loaded = false
    @State private var error: String?

    private var families: [(String, [RecordTypeSummary])] {
        let grouped = Dictionary(grouping: types) { DisplayNames.family($0.recordType) }
        return grouped.keys.sorted().map { key in
            (key, grouped[key, default: []].sorted { title($0) < title($1) })
        }
    }

    var body: some View {
        List {
            if let error { Section { ErrorText(message: error) } }
            if loaded && types.isEmpty {
                ContentUnavailableView("No records yet", systemImage: "tray",
                                       description: Text("Turn on a source on the home screen, then come back."))
            }
            ForEach(families, id: \.0) { family, rows in
                Section(family) {
                    ForEach(rows, id: \.self) { summary in
                        NavigationLink {
                            RecordListView(model: model, summary: summary, title: title(summary))
                        } label: {
                            TypeRow(summary: summary, title: title(summary))
                        }
                        .accessibilityIdentifier("record-type-\(summary.recordType)-\(summary.subtype ?? "")")
                    }
                }
            }
        }
        .navigationTitle("Your data")
        .task(id: model.dataVersion) { await load() }
        .refreshable { await model.collectNow(); await load() }
    }

    private func title(_ summary: RecordTypeSummary) -> String {
        summary.subtype.map(DisplayNames.subtype) ?? DisplayNames.type(summary.recordType)
    }

    private func load() async {
        do {
            types = try await model.recordTypes()
            error = nil
        } catch {
            self.error = error.userMessage
        }
        loaded = true
    }
}

private struct TypeRow: View {
    let summary: RecordTypeSummary
    let title: String

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack {
                Text(title).font(.body)
                Spacer()
                Text(summary.count.formatted()).font(.body.monospacedDigit()).foregroundStyle(.secondary)
            }
            Text(range).font(.caption).foregroundStyle(.secondary)
            if summary.deletedAtSource > 0 {
                Text("\(summary.deletedAtSource.formatted()) deleted at source").font(.caption).foregroundStyle(.orange)
            }
        }
    }

    private var range: String {
        let first = Date(millis: summary.firstStartMs).formatted(date: .abbreviated, time: .omitted)
        let last = Date(millis: summary.lastStartMs).formatted(date: .abbreviated, time: .omitted)
        let kind = summary.subtype != nil ? "\(DisplayNames.type(summary.recordType)) · " : ""
        return first == last ? "\(kind)\(first)" : "\(kind)\(first) – \(last)"
    }
}

/// Records of one type, newest first, grouped by day, loaded in pages.
struct RecordListView: View {
    let model: AppModel
    let summary: RecordTypeSummary
    let title: String
    @State private var rows: [RecordRow] = []
    @State private var exhausted = false
    @State private var loading = false
    @State private var error: String?

    private var days: [(Date, [RecordRow])] {
        let calendar = Calendar.current
        var out: [(Date, [RecordRow])] = []
        for row in rows {
            let day = calendar.startOfDay(for: Date(millis: row.startMs))
            if out.last?.0 == day {
                out[out.count - 1].1.append(row)
            } else {
                out.append((day, [row]))
            }
        }
        return out
    }

    var body: some View {
        List {
            if let error { Section { ErrorText(message: error) } }
            ForEach(days, id: \.0) { day, dayRows in
                Section(day.formatted(date: .complete, time: .omitted)) {
                    ForEach(dayRows, id: \.id) { row in
                        NavigationLink {
                            RecordDetailView(model: model, row: row, title: title)
                        } label: {
                            RecordRowView(row: row)
                        }
                        .accessibilityIdentifier("record-row")
                        .onAppear { if row.id == rows.last?.id { Task { await loadMore() } } }
                    }
                }
            }
            if loading { ProgressView().frame(maxWidth: .infinity) }
        }
        .navigationTitle(title)
        .navigationBarTitleDisplayMode(.inline)
        .task { if rows.isEmpty { await loadMore() } }
    }

    private func loadMore() async {
        guard !loading, !exhausted else { return }
        loading = true
        defer { loading = false }
        let after = rows.last.map { RecordCursor(startMs: $0.startMs, id: $0.id) }
        do {
            let page = try await model.records(type: summary.recordType, subtype: summary.subtype, after: after)
            rows.append(contentsOf: page)
            exhausted = page.count < 100
        } catch {
            self.error = error.userMessage
            exhausted = true
        }
    }
}

private struct RecordRowView: View {
    let row: RecordRow

    var body: some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack {
                Text(time).font(.subheadline.monospacedDigit())
                Spacer()
                if row.deletedAtSource {
                    Label("Deleted at source", systemImage: "trash").labelStyle(.iconOnly).foregroundStyle(.orange)
                }
            }
            if !row.preview.isEmpty {
                Text(row.preview).font(.footnote).foregroundStyle(.secondary).lineLimit(2)
            }
        }
    }

    private var time: String {
        let start = Date(millis: row.startMs).formatted(date: .omitted, time: .standard)
        guard let end = row.endMs, end != row.startMs else { return start }
        return "\(start) – \(Date(millis: end).formatted(date: .omitted, time: .standard))"
    }
}

/// One record's full JSON and provenance, read-only.
struct RecordDetailView: View {
    let model: AppModel
    let row: RecordRow
    let title: String
    @State private var json: String?
    @State private var error: String?

    var body: some View {
        ScrollView {
            VStack(alignment: .leading, spacing: 12) {
                VStack(alignment: .leading, spacing: 4) {
                    Text(title).font(.headline)
                    Text(Date(millis: row.startMs).formatted(date: .abbreviated, time: .standard))
                        .foregroundStyle(.secondary)
                    if row.deletedAtSource {
                        Label("Deleted at the source. Atlys keeps it.", systemImage: "trash")
                            .font(.footnote).foregroundStyle(.orange)
                    }
                }
                ErrorText(message: error)
                if let json {
                    ScrollView(.horizontal) {
                        Text(json)
                            .font(.caption.monospaced())
                            .padding(12)
                            .accessibilityIdentifier("record-json")
                    }
                    .background(.quaternary, in: .rect(cornerRadius: 10))
                } else if error == nil {
                    ProgressView()
                }
            }
            .padding()
        }
        .navigationTitle("Record")
        .navigationBarTitleDisplayMode(.inline)
        .task { await load() }
    }

    private func load() async {
        do {
            json = try await model.recordJSON(id: row.id) ?? "Record not found."
        } catch {
            self.error = error.userMessage
        }
    }
}
