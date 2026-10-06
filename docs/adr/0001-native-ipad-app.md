---
status: accepted
date: 2026-10-06
---

# A native iPad app on CloudKit, ported from the stocktake crate

The counters answered Q1 to Q3 in [questions-for-counters.md](../questions-for-counters.md): they'll count on iPads or phones because a laptop's battery doesn't last, up to four at once, scanning with the camera, over wifi that reaches the whole storage, with anyone who has the link able to count, and no login. The GPUI desktop app can't be that. We're building a native Swift iPad app, porting the rules in `crates/stocktake` by hand, and keeping the one shared stocktake in a CloudKit shared database, so there's no server to host. The desktop app is where the rules are worked out first: it builds and tests in seconds, so every decision the counters' answers settle lands there and is tried, and the layouts, flows and rules it ends up with are the design the iPad app ports. The port starts once the open questions are closed there.

## Considered options

- **Web app with a Rust server** (axum over the `stocktake` crate). Reaches every device, including the PC the counters might also use, and the crate is reused as is. Rejected because something has to host and run the server for a yearly event, and because a native app is the point of the project: it's an experiment, and the users are a handful of people in one storage, so App Store distribution isn't needed.
- **Keep GPUI, add sync and a web client.** Two UIs to keep in step, and GPUI has no iPad target. Rejected.
- **Rust core on the iPad through UniFFI.** Keeps one copy of the rules. Rejected because the rules are small, pure and well-tested, so porting them to Swift costs less than carrying a Rust toolchain and a bridge in an iOS project.
- **Firebase or Supabase for the shared store.** Easy link-based sharing, but a vendor account, and the counting rules re-expressed as security rules. Rejected in favour of CloudKit, which comes with the platform already chosen.

## Consequences

- The stocktake is a CloudKit record set shared with a `CKShare` whose public permission is read-write, so "anyone with the link" holds literally. Joining needs an iCloud account signed in on the device, which the storage's iPads have; that's the only identity, and the app doesn't show it.
- Counts are stored as their own records, one per saved count, not as a quantity on the product. Two counters saving at one location at the same time then never conflict, and the counted quantity is the sum of the records, which is the "counts are added" rule from Q3. Replacing a count writes a record that supersedes the earlier ones at that location.
- CloudKit queues writes while the device is offline and sends them when it can, so the stable-wifi assumption from Q2 is a convenience, not a requirement.
- Everything in `docs/spec.md` describes the iPad app's behaviour too; only the platform line changes. `docs/architecture.md` describes the desktop app, which stays the record of the design being ported.
- The tests in `crates/stocktake` are the port's acceptance tests: each one is rewritten in Swift against the ported rules.
