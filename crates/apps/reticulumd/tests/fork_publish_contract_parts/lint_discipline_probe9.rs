// Usage pins around the sequential peer-data lock order and its regression
// pin: the progress pin and its repeated-burst twin keep the shape that lets
// them see a deadlock (several loops, a multi-thread runtime, a bounded
// wait); the `dedupe` field names its only locker and that locker is the fn
// under the nest marker; the supervisor's `dedupe` field points at that
// rustdoc; no production source in any crate joins lock futures; every
// census the ledger states adds up and the one an Unreleased section states
// equals the census the tree carries; the probe sources and auto runtime
// test sections are ASCII and their headers carry no task or lane
// references; and the lifecycle header names each pin its file holds.
// Needles are assembled so this file never contains the attribute or marker
// text the sibling probes scan for.

/// The regression pin for the peer-data lock order.
const PEER_DATA_PROGRESS_PIN: &str = "crates/libs/rns-transport/src/iface/auto_runtime_parts/tests_sections/peer_data_receive_loop_progress.rs";

/// The repeated-burst twin of the progress pin, run under CPU pressure.
const PEER_DATA_PROGRESS_TWIN: &str = "crates/libs/rns-transport/src/iface/auto_runtime_parts/tests_sections/peer_data_receive_loop_progress_probe.rs";

/// The auto runtime test root that includes the pin.
const AUTO_RUNTIME_TESTS_ROOT: &str =
    "crates/libs/rns-transport/src/iface/auto_runtime_parts/tests.rs";

/// The file that defines the runtime whose `dedupe` field documents the order.
const AUTO_RUNTIME_PRELUDE: &str =
    "crates/libs/rns-transport/src/iface/auto_runtime_parts/module_prelude.rs";

/// The leading `//` comment block of a source file, whitespace-normalised.
fn leading_comment_block(text: &str) -> String {
    text.lines()
        .take_while(|line| line.starts_with("//"))
        .map(|line| line.trim_start_matches('/').trim())
        .collect::<Vec<_>>()
        .join(" ")
}

/// The first unsigned integer that follows `key` in `text`.
fn number_after(text: &str, key: &str) -> Option<usize> {
    let (_, rest) = text.split_once(key)?;
    let digits: String = rest.trim_start().chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// The `(total, transport, app)` a ledger section's census sentence states,
/// in the shape `back to <total> ... (<transport> in `rns-transport`, <app>
/// in the app crates)`; `None` when the section states no census. A section
/// that names the `rns-transport` split without the rest of the shape is a
/// malformed census, not a missing one, so that panics.
fn ledger_census(section: &str) -> Option<(usize, usize, usize)> {
    let flat = section.split_whitespace().collect::<Vec<_>>().join(" ");
    let (before, after) = flat.rsplit_once(" in `rns-transport`, ")?;
    let transport: usize = before
        .rsplit('(')
        .next()
        .and_then(|tail| tail.trim().parse().ok())
        .expect("a transport count precedes `rns-transport`");
    // With an empty key, number_after reads the number the remainder opens with.
    let app = number_after(after, "").expect("an app count follows");
    assert!(after.trim_start().contains("in the app crates"), "the split names the app crates");
    let total = before
        .rsplit_once("back to ")
        .and_then(|(_, tail)| number_after(tail, ""))
        .expect("a restored total precedes the split");
    Some((total, transport, app))
}

/// A deadlock between two receive loops needs two loops on two worker
/// threads and a wait that gives up; the pin keeps every one of those knobs,
/// so flipping it to a single socket or a current-thread runtime (which would
/// make it pass against any lock order) is refused here.
#[test]
fn peer_data_progress_pin_keeps_the_shape_that_can_see_a_deadlock() {
    let text = read_repo_file(PEER_DATA_PROGRESS_PIN);
    let attribute = text
        .lines()
        .find(|line| line.trim_start().starts_with("#[tokio::test("))
        .expect("the progress pin is a tokio test");
    assert!(
        attribute.contains("flavor = \"multi_thread\""),
        "the pin runs on a multi-thread runtime, got {attribute:?}"
    );
    let workers = number_after(attribute, "worker_threads =").expect("worker_threads is set");
    assert!(workers >= 2, "two loops need two workers to wait on each other, got {workers}");
    let sockets = number_after(&text, "const SOCKETS: usize =").expect("SOCKETS is a const");
    assert!(sockets >= 2, "a deadlock needs at least two receive loops, got {sockets}");
    let datagrams = number_after(&text, "const DATAGRAMS_PER_SOCKET: usize =")
        .expect("DATAGRAMS_PER_SOCKET is a const");
    assert!(datagrams >= 100, "a few hundred datagrams exercise the lock section, got {datagrams}");
    assert!(
        text.contains("spawn_peer_data_receive_loops("),
        "the pin drives the production fan-out, one loop per socket"
    );
    assert!(
        text.contains("timeout(std::time::Duration::from_secs(2)"),
        "the pin gives up after two seconds instead of hanging the suite"
    );
    assert!(
        text.contains("AutoPeerDataLoopEvent::Processed("),
        "the pin counts Processed events, the observable the loops emit"
    );
    let root = read_repo_file(AUTO_RUNTIME_TESTS_ROOT);
    assert!(
        root.contains("include!(\"tests_sections/peer_data_receive_loop_progress.rs\");"),
        "auto_runtime_parts/tests.rs includes the progress pin"
    );
}

/// One burst on an idle machine hits the cross-thread window only sometimes;
/// the twin makes the deadlock frequent with many independent rounds, several
/// loops on several workers, CPU pressure and a bounded wait, and every one
/// of those knobs is kept here. The per-socket depth is capped too: a stall
/// must mean a wedge, never a datagram the kernel dropped on loopback.
#[test]
fn strong_progress_twin_keeps_the_shape_that_makes_the_deadlock_frequent() {
    let text = read_repo_file(PEER_DATA_PROGRESS_TWIN);
    let attribute = text
        .lines()
        .find(|line| line.trim_start().starts_with("#[tokio::test("))
        .expect("the twin is a tokio test");
    assert!(
        attribute.contains("flavor = \"multi_thread\""),
        "the twin runs on a multi-thread runtime, got {attribute:?}"
    );
    let workers = number_after(attribute, "worker_threads =").expect("worker_threads is set");
    assert!(workers >= 2, "two loops need two workers to wait on each other, got {workers}");
    let rounds = number_after(&text, "const ROUNDS: usize =").expect("ROUNDS is a const");
    assert!(rounds >= 10, "the rounds are the independent trials, got {rounds}");
    let sockets = number_after(&text, "const SOCKETS: usize =").expect("SOCKETS is a const");
    assert!(sockets >= 2, "a deadlock needs at least two receive loops, got {sockets}");
    let backlog = number_after(&text, "const BACKLOG_PER_SOCKET: usize =")
        .expect("BACKLOG_PER_SOCKET is a const");
    let live =
        number_after(&text, "const LIVE_PER_SOCKET: usize =").expect("LIVE_PER_SOCKET is a const");
    assert!(
        backlog + live <= 160,
        "the per-socket depth stays below the Linux default receive buffer so a stall is never \
         a loopback drop (found {backlog} + {live})"
    );
    for needle in
        ["CpuPressure::start()", "spawn_peer_data_receive_loops(", "Duration::from_secs(2)"]
    {
        assert!(text.contains(needle), "the twin keeps {needle:?}");
    }
    let root = read_repo_file(AUTO_RUNTIME_TESTS_ROOT);
    assert!(
        root.contains("include!(\"tests_sections/peer_data_receive_loop_progress_probe.rs\");"),
        "auto_runtime_parts/tests.rs includes the twin"
    );
}

/// The `dedupe` field rustdoc names `spawn_peer_data_receive_loop` as the only
/// locker, that name is the fn carrying the nest marker, and the one
/// production `dedupe.lock()` site lives in that fn's file.
#[test]
fn dedupe_field_rustdoc_names_the_only_locker_and_the_name_resolves_to_the_marked_fn() {
    let prelude = read_repo_file(AUTO_RUNTIME_PRELUDE);
    let lines: Vec<&str> = prelude.lines().collect();
    let field = lines
        .iter()
        .position(|l| l.trim_start().starts_with("dedupe: Arc<tokio::sync::Mutex<"))
        .expect("the runtime has a dedupe mutex field");
    let doc = lines[..field]
        .iter()
        .rev()
        .take_while(|l| l.trim_start().starts_with("///"))
        .map(|l| l.trim_start().trim_start_matches("///").trim())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" ");
    assert!(
        doc.contains("Lock order: `state`, then `dedupe`"),
        "the dedupe field states the order verbatim, got {doc:?}"
    );
    assert!(
        doc.contains("`spawn_peer_data_receive_loop` is the only locker of `dedupe`"),
        "the dedupe field names its only locker, got {doc:?}"
    );

    let loop_file = "crates/libs/rns-transport/src/iface/auto_runtime_parts/autoruntimeplan_sections/spawn_peer_data_receive_loop.rs";
    let text = read_repo_file(loop_file);
    let lines: Vec<&str> = text.lines().collect();
    let marker = lines
        .iter()
        .position(|l| is_site_allowance(l) && l.contains(OWNER_TAG))
        .expect("the loop carries the owner-tagged allowance");
    let item = lines[marker + 1..]
        .iter()
        .find(|l| !l.trim_start().starts_with("#["))
        .expect("an item follows the marker");
    assert!(
        item.contains("fn spawn_peer_data_receive_loop("),
        "the allowance sits on the fn the rustdoc names, got {item:?}"
    );
    assert!(
        !item.trim_start().starts_with("pub"),
        "the marked fn is private, so the fix changes no public signature"
    );
    let dedupe_sites: Vec<String> = tracked_rust_sources()
        .into_iter()
        .filter(|p| !is_test_or_example_path(p))
        .filter(|p| read_repo_file(p).contains("dedupe.lock()"))
        .collect();
    assert_eq!(
        dedupe_sites,
        vec![loop_file.to_string()],
        "dedupe has exactly one production locker"
    );
}

/// The supervisor holds the same `dedupe` mutex but documents no order of its
/// own: its one rustdoc line sends the reader to the runtime field that does.
#[test]
fn supervisor_dedupe_field_points_at_the_runtime_lock_order_doc() {
    let prelude = read_repo_file(AUTO_RUNTIME_PRELUDE);
    let lines: Vec<&str> = prelude.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.starts_with("pub struct AutoPeerDataListenerSupervisor"))
        .expect("the prelude defines the supervisor");
    let end = start
        + lines[start..]
            .iter()
            .position(|l| l.trim() == "}")
            .expect("the supervisor struct closes");
    let field = start
        + lines[start..end]
            .iter()
            .position(|l| l.trim_start().starts_with("dedupe:"))
            .expect("the supervisor has a dedupe field");
    let doc: Vec<&str> = lines[start..field]
        .iter()
        .rev()
        .take_while(|l| l.trim_start().starts_with("///"))
        .map(|l| l.trim_start().trim_start_matches("///").trim())
        .collect();
    assert_eq!(doc.len(), 1, "the supervisor field carries one rustdoc line, got {doc:?}");
    for needle in ["`AutoPeerDataReceiveLoopRuntime::dedupe`", "lock order"] {
        assert!(doc[0].contains(needle), "the pointer says {needle:?}, got {:?}", doc[0]);
    }
}

/// Joining two lock futures parks the first-won guard while the other pends
/// and rotates the poll order, so two tasks sharing the pair can wait on each
/// other; no production source in any crate does it, not only the auto
/// runtime.
#[test]
fn no_production_source_in_any_crate_joins_lock_futures() {
    let join_macros = ["join!(", "try_join!("];
    let lock_calls = [".lock()", ".read()", ".write()", ".lock_owned()"];
    let mut offenders: Vec<String> = Vec::new();
    for path in tracked_rust_sources() {
        if is_test_or_example_path(&path) {
            continue;
        }
        let text = read_repo_file(&path);
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            if !join_macros.iter().any(|m| line.contains(m)) {
                continue;
            }
            let window = lines[index..(index + 4).min(lines.len())].join("\n");
            if lock_calls.iter().any(|call| window.contains(call)) {
                offenders.push(format!("{path}:{}", index + 1));
            }
        }
    }
    assert!(offenders.is_empty(), "production code joins lock futures at {offenders:?}");
}

/// Every PATCHSET census sentence splits its total by crate, and the split
/// adds up to the total in every section that states one. Released sections
/// are frozen, so this arithmetic is all they are held to.
#[test]
fn ledger_census_sentences_are_arithmetically_consistent() {
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let labels: Vec<&str> = ledger
        .lines()
        .filter_map(|line| line.strip_prefix("## ")?.split_once(" (upstream ").map(|(l, _)| l))
        .collect();
    for label in labels {
        let section = ledger_section(&ledger, label)
            .unwrap_or_else(|| panic!("the `## {label}` heading should open a ledger section"));
        let Some((total, transport, app)) = ledger_census(section) else { continue };
        assert_eq!(
            transport + app,
            total,
            "the {label} census split ({transport} + {app}) adds up to its total"
        );
    }
}

/// The census an Unreleased section states equals what the tree carries, so
/// the ledger cannot vouch for a count the code no longer has. The live
/// comparison re-arms when the next cycle's ledger opens a section that
/// states a census; released sections are checked for arithmetic only.
#[test]
fn unreleased_census_sentence_matches_the_measured_production_allowances() {
    let mut transport = 0_usize;
    let mut app = 0_usize;
    for path in tracked_rust_sources() {
        if is_test_or_example_path(&path) {
            continue;
        }
        let sites = read_repo_file(&path).lines().filter(|l| is_site_allowance(l)).count();
        if path.starts_with("crates/libs/rns-transport/src/") {
            transport += sites;
        } else {
            app += sites;
        }
    }
    let ledger = read_repo_file("docs/fork/PATCHSET.md");
    let Some(section) = ledger_section(&ledger, "Unreleased") else { return };
    let Some((total, stated_transport, stated_app)) = ledger_census(section) else { return };
    assert_eq!(
        (total, stated_transport, stated_app),
        (transport + app, transport, app),
        "the Unreleased census matches the tree"
    );
}

/// Probe sources and auto runtime test sections are ASCII, and no probe
/// header under `fork_publish_contract_parts/` carries a task or round
/// reference, whichever contract the probe covers: the files describe the
/// contract they pin, not the lane that wrote them.
#[test]
fn probe_sources_and_test_sections_are_ascii_with_lane_free_headers() {
    let task_token = concat!("TASK", "-");
    let lane_tokens = [" round ", "Round ", "gauntlet", "probe-lane", "orchestrator"];
    let mut non_ascii: Vec<String> = Vec::new();
    let mut lane_refs: Vec<String> = Vec::new();
    for path in git_tracked_files() {
        let in_scope = (path.starts_with("crates/apps/reticulumd/tests/fork_publish_contract")
            || path.starts_with("crates/libs/rns-transport/src/iface/auto_runtime_parts/tests"))
            && path.ends_with(".rs");
        if !in_scope {
            continue;
        }
        let text = read_repo_file(&path);
        if !text.is_ascii() {
            non_ascii.push(path.clone());
        }
        if path.contains("fork_publish_contract_parts/") && path.contains("_probe") {
            let header = leading_comment_block(&text);
            if header.contains(task_token) || lane_tokens.iter().any(|t| header.contains(t)) {
                lane_refs.push(path.clone());
            }
        }
    }
    assert!(non_ascii.is_empty(), "non-ASCII bytes in {non_ascii:?}");
    assert!(lane_refs.is_empty(), "probe headers carry lane references: {lane_refs:?}");
}

/// The lifecycle fixture header names every pin the file holds, including the
/// three announce and outbound pins, and each named pin exists as a test.
#[test]
fn lifecycle_fixture_header_names_the_announce_and_outbound_pins_it_holds() {
    let path =
        "crates/libs/rns-transport/src/transport/tests_parts/two_transport_loopback_lifecycle.rs";
    let text = read_repo_file(path);
    let header = leading_comment_block(&text);
    let pins = [
        ("buildable announce", "fn send_announce_hands_a_buildable_announce_to_the_interface("),
        (
            "pathless outbound packet broadcast",
            "fn outbound_without_a_path_broadcasts_when_configured(",
        ),
        (
            "dropped when broadcast is off",
            "fn outbound_without_a_path_sends_nothing_when_broadcast_is_off(",
        ),
        (
            "`try_send_announce` surfacing the build error as `Err`",
            "fn try_send_announce_reports_the_build_error(",
        ),
    ];
    for (phrase, test_fn) in pins {
        assert!(header.contains(phrase), "{path} header names the pin {phrase:?}, got {header:?}");
        assert!(text.contains(test_fn), "{path} holds the test the header names: {test_fn}");
    }
}
