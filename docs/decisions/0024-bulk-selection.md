# 0024: Keep bulk selections in Rust

- Status: accepted (2026-10-22)

Select all freezes matching cached conversation keys in Rust, including
unloaded rows. A single replaceable snapshot owns the sorted, deduplicated
account/thread pairs; no message bodies are copied. The UI holds a token,
count and unchecked exceptions. It checks membership for loaded rows only;
new arrivals stay outside the snapshot. Folder, account, category or query
changes clear selection and stop subsequent mutation batches.

Local searches use the same supported predicates as the cache. Queries
requiring Gmail select only the loaded results, clearly labelled; this does
not claim to select an entire uncached remote mailbox. No new network
endpoints, scopes, dependencies or polling loops are required.

Mark-read batches contain at most ten keys. Calls run sequentially, paced
to at most ten writes/second, with progress and Stop between batches. Rust
confirms each remote write before updating the cache, emits one update per
batch, and stops on failure. Previously read messages are skipped; failures
never use an inverse label change that could mark read messages unread.
This action is not an atomic transaction across Gmail conversations.
