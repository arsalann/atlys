# Market lessons

What happened to earlier attempts at personal data ownership, and what the relaunch should do differently.

## Pattern 1: the two-sided "personal data exchange" stalls

**Examples:** digi.me, HAT/Dataswyft, Solid/Inrupt (consumer side), MyData operators.

These projects promised that users would control their data and businesses would pay to access it with consent. They needed users *and* businesses at the same time. Users got little day-to-day value until businesses showed up, and businesses waited for users. Inrupt pivoted toward enterprise servers.

**Lesson:** Atlys has to be useful to a single user, with zero businesses involved. The personal AI assistant is that single-player value. The protocol and business ecosystem come second.

## Pattern 2: vendor-held personal memory is fragile

**Examples:** Rewind/Limitless (acquired by Meta Dec 2025; desktop app wound down, pendant discontinued, users asked to accept new terms); Microsoft Recall (privacy backlash at launch).

Users who trusted a startup with a full record of their life saw that record's future decided by an acquisition.

**Lesson:** keep personal context in a vault the user holds, under open source and an open format. The Atlys message: *your AI's memory of you should outlive any AI company.*

## Pattern 3: open source importers thrive but stay niche

**Examples:** HPI, Dogsheep, Timelinize, Perkeep.

They are loved by technical users and have rich parser libraries. They stay niche because setup is technical, there are no mobile apps, no encryption (they assume a trusted machine), and no "what do I do with it now?" payoff.

**Lesson:** Atlys needs (a) mobile apps that collect data with zero setup, (b) encryption by default, and (c) an AI agent as the payoff. Collaborate with these projects instead of competing: reuse parsers and credit them.

## Pattern 4: crypto-economic data ownership

**Example:** Vana (data DAOs, token incentives, user-owned models).

It attracted users (the Reddit DAO had ~140k contributors) through financial incentives, but it optimizes for *pooling* data, which is the opposite of private.

**Lesson:** monetization through sharing/selling is a valid later layer, but it must be opt-in and separate from the private core. Don't let a token model shape the architecture.

## Pattern 5: deletion services work as authorized agents

**Examples:** Permission Slip (~5M requests, now part of DeleteMe), DeleteMe/Incogni/Optery, California DROP.

Deletion has proven consumer demand, and regulation is turning it into a state-provided service (DROP).

**Lesson:** deletion is a feature, not the product. Integrate with DROP, generate requests, and track them, but don't try to become a broker-removal company.

## What the relaunch should take from all this

1. **Single-player value first:** a vault plus your own AI assistant.
2. **Zero-knowledge and open:** trust through architecture, not promises.
3. **Mobile collection with zero setup:** the original Atlys's strength, now cross-platform.
4. **An open schema and MCP:** be the layer others build on.
5. **Community connectors:** the long tail of sources can only scale through contributors.
