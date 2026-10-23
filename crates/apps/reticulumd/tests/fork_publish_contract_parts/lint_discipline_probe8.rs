// Pins on the shape of the peer-data receive loop and its neighbours: the
// "documented lock-order nest" allowance category has exactly one member and
// it sits on `spawn_peer_data_receive_loop`, whose body takes `state` and
// then `dedupe` in sequence (one loop per adopted NIC shares both mutexes, so
// a `tokio::join!` over the two lock futures can park one guard while the
// other pends and let two loops wait on each other); the order is written on
// the runtime's `dedupe` field; `link_table_cleanup` carries no stale
// `dead_code` allow; the lock-discipline rustdoc names the nests by fn
// instead of line ranges; the README divergence register names the unicast
// discovery-port step-down with its Python cite; and the serial
// transmit-join probe root carries no allowance it never needed. Needles are
// assembled so this file never contains the attribute text the sibling
// probes scan for.

/// The category phrase with exactly one live member.
const LOCK_ORDER_NEST_PHRASE: &str = "documented lock-order nest";

/// The `dead_code` allowance attribute, assembled.
fn dead_code_allow() -> String {
    format!("#[allow({}_code)]", "dead")
}

/// The one production file that nests the `state` and `dedupe` guards.
const PEER_DATA_LOOP: &str = "crates/libs/rns-transport/src/iface/auto_runtime_parts/autoruntimeplan_sections/spawn_peer_data_receive_loop.rs";

/// The lock-order-nest category has exactly one production member, the
/// marker directly above `fn spawn_peer_data_receive_loop(`, and the order it
/// documents is written on the runtime's `dedupe` field.
#[test]
fn exactly_one_production_allowance_claims_the_lock_order_nest_and_it_sits_on_the_peer_data_loop() {
    let mut sites = 0_usize;
    let mut nests: Vec<String> = Vec::new();
    for path in tracked_rust_sources() {
        if is_test_or_example_path(&path) {
            continue;
        }
        let text = read_repo_file(&path);
        for (index, line) in text.lines().enumerate() {
            if !is_site_allowance(line) {
                continue;
            }
            sites += 1;
            if line.contains(LOCK_ORDER_NEST_PHRASE) {
                nests.push(format!("{path}:{}", index + 1));
            }
        }
    }
    assert!(sites >= 50, "expected the measured production allowances, found {sites}");
    assert_eq!(nests.len(), 1, "the lock-order-nest category has one member, found {nests:?}");
    assert!(nests[0].starts_with(PEER_DATA_LOOP), "{nests:?}");

    let text = read_repo_file(PEER_DATA_LOOP);
    let lines: Vec<&str> = text.lines().collect();
    let marker = lines
        .iter()
        .position(|l| is_site_allowance(l) && l.contains(LOCK_ORDER_NEST_PHRASE))
        .expect("the peer-data loop carries the nest marker");
    let item = lines[marker + 1..]
        .iter()
        .find(|l| !l.trim_start().starts_with("#["))
        .expect("an item follows the marker");
    assert!(
        item.contains("fn spawn_peer_data_receive_loop("),
        "the marker sits on spawn_peer_data_receive_loop, got {item:?}"
    );

    let prelude = read_repo_file(
        "crates/libs/rns-transport/src/iface/auto_runtime_parts/module_prelude.rs",
    );
    let lines: Vec<&str> = prelude.lines().collect();
    let start = lines
        .iter()
        .position(|l| l.contains("struct AutoPeerDataReceiveLoopRuntime"))
        .expect("AutoPeerDataReceiveLoopRuntime in module_prelude.rs");
    let end = lines[start..]
        .iter()
        .position(|l| l.starts_with('}'))
        .map(|offset| start + offset)
        .expect("the struct body closes");
    let field = lines[start..end]
        .iter()
        .position(|l| l.trim_start().starts_with("dedupe:"))
        .map(|offset| start + offset)
        .expect("the runtime has a dedupe field");
    let doc = lines[start..field]
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
        ["Lock order:", "state", "then", "dedupe"].iter().all(|needle| doc.contains(needle)),
        "the dedupe field documents the state-then-dedupe order, got {doc:?}"
    );
}

/// The peer-data receive loop takes `state` and then `dedupe` in sequence
/// under its one nest marker, with no stale `dead_code` allow and no
/// `tokio::join!` over lock futures anywhere in the auto runtime. The
/// `dedupe` mutex has exactly one production locker, so the nest is a total
/// order and no other site can close a cycle against it.
#[test]
fn peer_data_receive_loop_takes_state_then_dedupe_sequentially_under_its_marker() {
    let text = read_repo_file(PEER_DATA_LOOP);
    let markers: Vec<&str> = text.lines().filter(|l| is_site_allowance(l)).collect();
    assert_eq!(markers.len(), 1, "the peer-data loop carries one allowance, found {markers:?}");
    assert!(markers[0].contains(LOCK_ORDER_NEST_PHRASE), "{markers:?}");
    assert!(
        !text.contains(&dead_code_allow()),
        "the stale dead_code allow on spawn_peer_data_receive_loop is gone"
    );
    let state_locks: Vec<usize> = text
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains("state.lock().await"))
        .map(|(index, _)| index)
        .collect();
    let dedupe_locks: Vec<usize> = text
        .lines()
        .enumerate()
        .filter(|(_, l)| l.contains("dedupe.lock().await"))
        .map(|(index, _)| index)
        .collect();
    assert_eq!(state_locks.len(), 1, "state is locked once, at lines {state_locks:?}");
    assert_eq!(dedupe_locks.len(), 1, "dedupe is locked once, at lines {dedupe_locks:?}");
    assert!(
        state_locks[0] < dedupe_locks[0],
        "state is taken before dedupe: state at {state_locks:?}, dedupe at {dedupe_locks:?}"
    );

    let mut joined_locks: Vec<String> = Vec::new();
    let mut dedupe_lockers: Vec<String> = Vec::new();
    for path in tracked_rust_sources() {
        if !path.starts_with("crates/libs/rns-transport/src/") || is_test_or_example_path(&path) {
            continue;
        }
        if path.contains("_probe") {
            continue;
        }
        let text = read_repo_file(&path);
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            if line.contains("dedupe.lock()") {
                dedupe_lockers.push(format!("{path}:{}", index + 1));
            }
            if path.contains("/auto_runtime_parts/")
                && (line.contains("tokio::join!(") || line.contains("tokio::try_join!("))
                && lines[index..(index + 3).min(lines.len())].iter().any(|l| l.contains(".lock()"))
            {
                joined_locks.push(format!("{path}:{}", index + 1));
            }
        }
    }
    assert!(
        joined_locks.is_empty(),
        "no join! in the auto runtime takes lock futures together, found {joined_locks:?}"
    );
    assert_eq!(
        dedupe_lockers.len(),
        1,
        "the dedupe mutex has exactly one production locker, found {dedupe_lockers:?}"
    );
    assert!(dedupe_lockers[0].starts_with(PEER_DATA_LOOP), "{dedupe_lockers:?}");
}

/// `handle_link_table_cleanup` is live (`manage_transport` calls it) so its
/// `dead_code` allow was stale; the file opens with the owner-tagged
/// handler-guard marker and nothing else.
#[test]
fn link_table_cleanup_opens_with_its_marker_and_carries_no_dead_code_allow() {
    let path = "crates/libs/rns-transport/src/transport/jobs_parts/link_table_cleanup.rs";
    let text = read_repo_file(path);
    assert!(!text.contains(&dead_code_allow()), "{path}: stale dead_code allow is gone");
    let first = text.lines().find(|l| !l.trim().is_empty()).unwrap_or_default();
    assert!(
        is_site_allowance(first) && first.contains(OWNER_TAG),
        "{path}: the first line is the owner-tagged allowance, got {first:?}"
    );
    let caller = read_repo_file("crates/libs/rns-transport/src/transport/jobs.rs");
    assert!(
        caller.contains("handle_link_table_cleanup("),
        "the cleanup job is live: jobs.rs calls handle_link_table_cleanup"
    );
}

/// The lock-discipline rustdoc names the nested fns rather than citing line
/// ranges that drift, and every fn it names exists under `transport/`.
#[test]
fn lock_discipline_rustdoc_names_fns_that_exist_and_cites_no_line_ranges() {
    let text = read_repo_file("crates/libs/rns-transport/src/transport/mod.rs");
    let lines: Vec<&str> = text.lines().collect();
    let at = lines
        .iter()
        .position(|l| l.starts_with("pub(crate) struct TransportHandler"))
        .expect("TransportHandler definition in transport/mod.rs");
    let doc = lines[..at]
        .iter()
        .rev()
        .take_while(|l| l.starts_with("///"))
        .map(|l| l.trim_start_matches("///").trim())
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect::<Vec<_>>()
        .join(" ");
    let discipline = doc
        .split_once("# Lock discipline")
        .map(|(_, rest)| rest)
        .expect("the rustdoc has a `# Lock discipline` section");

    // A `.rs:NN` or `.rs:NN-MM` cite inside the discipline prose.
    let line_cites: Vec<&str> = discipline
        .split(|c: char| c.is_whitespace() || matches!(c, '(' | ')' | '`' | ',' | ';'))
        .filter(|tok| {
            tok.split_once(".rs:")
                .is_some_and(|(_, tail)| tail.starts_with(|c: char| c.is_ascii_digit()))
        })
        .collect();
    assert!(
        line_cites.is_empty(),
        "the lock discipline names fns, not line ranges that drift: {line_cites:?}"
    );

    // Every backticked snake_case name that reads as a fn resolves to an
    // `fn <name>` somewhere under transport/.
    let named: Vec<&str> = discipline
        .split('`')
        .skip(1)
        .step_by(2)
        .filter(|tok| {
            tok.chars().all(|c| c.is_ascii_lowercase() || c == '_')
                && tok.contains('_')
                && !tok.ends_with(".rs")
        })
        .collect();
    assert!(
        named.contains(&"handle_check_links")
            && named.contains(&"handle_link_resource_packet")
            && named.contains(&"note_link_packet_sent"),
        "the discipline names the nesting fns, got {named:?}"
    );
    let transport_sources: Vec<String> = tracked_rust_sources()
        .into_iter()
        .filter(|p| p.starts_with("crates/libs/rns-transport/src/transport/"))
        .collect();
    let field_names = ["packet_cache", "iface_manager", "max_request_size"];
    for name in named {
        if field_names.contains(&name) {
            continue;
        }
        let needle = format!("fn {name}");
        assert!(
            transport_sources.iter().any(|p| read_repo_file(p).contains(&needle)),
            "the lock discipline names `{name}` but no `fn {name}` exists under transport/"
        );
    }
}

/// The README divergence register carries the unicast discovery-port
/// step-down as its own entry with the Python cite the fix round named.
#[test]
fn readme_register_names_the_unicast_port_step_down_with_its_cite() {
    let readme = read_repo_file("crates/libs/rns-transport/README.md");
    let (_, section) = readme
        .split_once("Known divergences from Python RNS")
        .expect("the README has the divergence register");
    let section = section.split("\nNot divergences:").next().unwrap_or(section);
    let entry = section
        .split("\n- ")
        .map(|e| e.split_whitespace().collect::<Vec<_>>().join(" "))
        .find(|e| e.contains("unicast_discovery_port"))
        .expect("a register entry names AutoInterfaceConfig::unicast_discovery_port");
    for required in ["65534", "65535", "`RNS/Interfaces/AutoInterface.py:173`"] {
        assert!(entry.contains(required), "the step-down entry says {required:?}, got {entry:?}");
    }
    let changelog = read_repo_file("crates/libs/rns-transport/CHANGELOG.md");
    let normalised = changelog.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        normalised.contains("unicast_discovery_port") && normalised.contains("65534"),
        "the CHANGELOG carries the same step-down under its fix bullets"
    );
}

/// A root allowance is only sanctioned on a root that holds a guard across
/// an await; the serial transmit-join probe never binds a tokio guard, so
/// its allowance was dropped and must not come back.
#[test]
fn serial_transmit_join_probe_root_binds_no_tokio_guard_and_carries_no_allowance() {
    let path = "crates/libs/rns-transport/src/iface/serial_parts/transmit_join_probe_tests.rs";
    let text = read_repo_file(path);
    assert!(
        !text.contains(&module_level_allow()),
        "{path}: a root that holds no guard across an await carries no allowance"
    );
    assert!(
        !text.contains(".lock().await")
            && !text.contains(".read().await")
            && !text.contains(".write().await"),
        "{path}: the probe binds no tokio guard, so nothing here can span an await"
    );
}
