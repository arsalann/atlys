import AtlysCollectors
import Testing

@Test("The v0 plan defines four collectors")
func fourCollectors() {
    #expect(CollectorID.allCases.map(\.rawValue) == ["health", "location", "motion", "device"])
}
