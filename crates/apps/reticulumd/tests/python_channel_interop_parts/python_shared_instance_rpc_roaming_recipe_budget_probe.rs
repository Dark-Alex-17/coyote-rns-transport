// The README `## Driving rnsd's path table` recipe followed step by step by
// a consumer on the roaming topology of `python_shared_instance_rpc_roaming.rs`,
// whose `roaming_topology` fixture and helpers this part reuses,
// with the two steps the roaming proof cites but does not exercise:
//
//  * "find the row. Its `interface` ... equals the `interface_stats` entry's
//    `name` ... If `online == false` ... call `drop_path_settled`": the stats
//    entry is looked up by `name == row.interface` (not by `short_name`) and
//    read as offline before the drop;
//  * "retry the link with `first_hop_timeout` as the budget for the first
//    hop": rnsd's own `first_hop_timeout` figure is read over RPC before each
//    link, and the link over the dead row closes with `Timeout` inside
//    Python's `first_hop_timeout + 6 s * max(1, hops)` (`RNS/Link.py:282-283`)
//    plus the maintenance slack, no earlier than 6 s + 6 s per hop, while the
//    link over the live route activates inside the same budget. That pins
//    the first-hop sizing of `transport_sections/link_establishment.rs`
//    against Python's figure live: a 262144-byte first hop would take about 45 s.
//
// Also pinned: a consumer that retries `request_path` once more after the
// recovery (a duplicate retry) leaves the row on B at hops 3; rnsd answers
// the repeat from its fresh row (`RNS/Transport.py:3459`) and A, offline,
// never comes back into it.
//
// Unix only like its siblings. Imports and helpers come from the roaming
// part, the paths-live part and the stdlib-listener probe.

#[cfg(unix)]
const RECIPE_BUDGET_TOTAL: Duration = Duration::from_secs(120);
#[cfg(unix)]
const RECIPE_BUDGET_PER_HOP: f64 = 6.0;
// The maintenance job closes a timed-out link on its own cadence; the
// roaming proof saw 0.85 s past the figure.
#[cfg(unix)]
const RECIPE_BUDGET_SLACK: Duration = Duration::from_millis(2_500);

/// Python's establishment timeout for `hops` with the first hop at `first_hop`.
#[cfg(unix)]
fn recipe_establishment_budget(first_hop: f64, hops: u8) -> Duration {
    Duration::from_secs_f64(first_hop + RECIPE_BUDGET_PER_HOP * f64::from(hops.max(1)))
}

/// The `interface_stats` entry whose `name` is `row.interface`, the README's lookup.
#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn recipe_stats_entry_for_row(
    endpoint: &RpcEndpoint,
    key: &RpcKey,
    row: &PathRow,
) -> InterfaceStat {
    let stats = interface_stats(endpoint, key, PATH_LIVE_CALL).await.expect("interface_stats");
    stats.interfaces.iter().find(|iface| iface.name == row.interface).cloned().unwrap_or_else(
        || panic!("no stats entry named {:?} in {:?}", row.interface, stats.interfaces),
    )
}

#[cfg(unix)]
#[deny(clippy::await_holding_invalid_type)]
async fn recipe_budget_scenario(
    started: tokio::time::Instant,
    config_dir: &std::path::Path,
    rnsd_slot: &mut Option<SharedInstanceRpcRnsd>,
) {
    // ---- Topology, as the roaming proof builds it. ----
    let RoamingTopology {
        r2, r1, d, c, a_server, b_name, hash, endpoint, key, c_hops_via_a, ..
    } = roaming_topology("recipe", config_dir, SharedInstanceOptions::default(), rnsd_slot).await;
    let rnsd = rnsd_slot.as_mut().expect("the fixture spawned rnsd");

    // ---- A goes down. The README's lookup: the stats entry named like the row. ----
    assert!(d.stop_interface(a_server).await, "D's A listener stops");
    roaming_wait_online(&endpoint, &key, "A", false, ROAMING_A_OFFLINE).await;
    let rows = path_table(&endpoint, &key, None, PATH_LIVE_CALL).await.expect("path_table");
    let row = path_live_row(&rows, &hash).expect("the row survives A going offline");
    let entry = recipe_stats_entry_for_row(&endpoint, &key, &row).await;
    assert_eq!(entry.short_name, "A", "the row's interface str names A's entry: {entry:?}");
    assert!(!entry.online, "the README's trigger: the row's interface reads offline: {entry:?}");

    // ---- The failed link, timed against rnsd's own first_hop_timeout. ----
    let first_hop_a = first_hop_timeout(&endpoint, &key, &hash, PATH_LIVE_CALL)
        .await
        .expect("first_hop_timeout over the dead row");
    assert!(first_hop_a.is_finite() && first_hop_a >= 6.0, "{first_hop_a}");
    let c_hops = c.hops_to(&hash).await;
    assert_eq!(c_hops, c_hops_via_a);
    let budget_a = recipe_establishment_budget(first_hop_a, c_hops);
    let desc = c.destination_desc(&hash).await.expect("C holds D's description");
    let mut link_events = c.out_link_events();
    let link_started = tokio::time::Instant::now();
    let link = c.link(desc).await.expect("Ok: a path exists");
    let reason =
        wait_for_out_link_closed(&mut link_events, &link, budget_a + RECIPE_BUDGET_SLACK).await;
    let failed_after = link_started.elapsed();
    eprintln!(
        "OBSERVED: rnsd first_hop_timeout over A = {first_hop_a} s, C hops {c_hops}, Python budget \
         {budget_a:?}; link over the dead row closed {reason:?} after {failed_after:?}"
    );
    assert_eq!(reason, Some(LinkCloseReason::Timeout));
    assert!(
        failed_after
            >= Duration::from_secs_f64(6.0 + RECIPE_BUDGET_PER_HOP * f64::from(c_hops.max(1))),
        "Python's floor is 6 s + 6 s per hop: {failed_after:?}"
    );
    assert!(
        failed_after <= budget_a + RECIPE_BUDGET_SLACK,
        "the link over the dead row closes inside Python's figure {budget_a:?} plus slack, \
         not the ~45 s a 262144-byte first hop gives: {failed_after:?}"
    );

    // ---- The recipe: drop_path_settled, one request_path, retry the link. ----
    assert!(roaming_stats_entry(&endpoint, &key, "B").await.online);
    assert!(
        drop_path_settled(&endpoint, &key, &hash, PATH_LIVE_CALL).await.expect("drop_path_settled"),
        "the stale row settles"
    );
    // C already expired its own row on the establishment timeout (a shared-instance
    // client queues no rediscovery), so the next accepted announce is an event.
    roaming_wait_path(&c, &hash, false, ROAMING_REPATH, "C drops the row of the timed-out link")
        .await;
    let mut announces = c.recv_announces().await;
    let dispatch = c.request_path(&hash, None, None).await;
    assert!(dispatch.sent_ifaces + dispatch.queued_ifaces >= 1, "{dispatch:?}");
    let row_via_b = roaming_wait_row(&endpoint, &key, &hash, 3, &b_name, ROAMING_BRING_UP).await;
    let entry_b = recipe_stats_entry_for_row(&endpoint, &key, &row_via_b).await;
    assert_eq!(entry_b.short_name, "B", "{entry_b:?}");
    assert!(entry_b.online, "{entry_b:?}");
    let (c_hops_via_b, _, _) =
        path_recipe_wait_announce(&mut announces, hash, ROAMING_BRING_UP).await;
    assert_eq!(c_hops_via_b, 3);
    let first_hop_b = first_hop_timeout(&endpoint, &key, &hash, PATH_LIVE_CALL)
        .await
        .expect("first_hop_timeout over B");
    assert!(first_hop_b.is_finite() && first_hop_b >= 6.0, "{first_hop_b}");
    let budget_b = recipe_establishment_budget(first_hop_b, c_hops_via_b);
    let mut link_events = c.out_link_events();
    let link_started = tokio::time::Instant::now();
    let link = c.link(desc).await.expect("Ok: the B route is a path");
    wait_for_out_link_active(&mut link_events, &link, budget_b).await;
    let active_after = link_started.elapsed();
    eprintln!(
        "OBSERVED: rnsd first_hop_timeout over B = {first_hop_b} s, C hops {c_hops_via_b}, budget \
         {budget_b:?}; link over B active after {active_after:?}"
    );

    // ---- A duplicate retry of the recipe's request leaves the row on B. ----
    let log_before = rnsd._log.snapshot().len();
    let dispatch = c.request_path(&hash, None, None).await;
    assert!(dispatch.sent_ifaces + dispatch.queued_ifaces >= 1, "{dispatch:?}");
    let known = rnsd
        ._log
        .wait_for("the repeat answered from the B row", ROAMING_REPATH, |line| {
            (line.contains(PATH_RECIPE_KNOWN_LINE)
                && line.contains(&format!("<{}>", hex::encode(hash.as_slice()))))
            .then(|| line.to_string())
        })
        .await;
    eprintln!("rnsd: {known}");
    tokio::time::sleep(PATH_LIVE_SETTLE).await;
    let rows = path_table(&endpoint, &key, None, PATH_LIVE_CALL).await.expect("path_table");
    let row_after = path_live_row(&rows, &hash).expect("the row stays");
    assert_eq!(row_after.interface, b_name, "a repeat request never files A again: {row_after:?}");
    assert_eq!(row_after.hops, 3, "{row_after:?}");
    assert_eq!(
        next_hop_interface_name(&endpoint, &key, &hash, PATH_LIVE_CALL).await.expect("next hop"),
        Some(b_name.clone())
    );
    let since = rnsd._log.snapshot();
    let since = &since[log_before.min(since.len())..];
    assert!(
        path_recipe_lines_for(
            since,
            PATH_RECIPE_FORWARDED_LINE,
            &format!("<{}>", hex::encode(hash.as_slice()))
        )
        .is_empty(),
        "with the B row on file the repeat is answered, not forwarded"
    );
    assert_eq!(c.hops_to(&hash).await, 3, "C's row stays at the B hop count");

    let detached = tokio::time::timeout(ROAMING_TEARDOWN, c.detach_shared_instance())
        .await
        .expect("detach concludes");
    assert!(detached);
    for transport in [&c, &d, &r1, &r2] {
        tokio::time::timeout(ROAMING_TEARDOWN, transport.stop()).await.expect("stops");
    }
    rnsd.assert_running("at the end");
    eprintln!("OBSERVED: recipe budget probe total {:?}", started.elapsed());
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "requires local Python Reticulum checkout"]
async fn python_rnsd_roaming_recipe_links_inside_pythons_first_hop_timeout_budget_and_a_repeat_request_stays_on_b(
) {
    let _interop_guard = python_interop_guard().await;
    let started = tokio::time::Instant::now();
    let temp = tempfile::tempdir().expect("tempdir");
    let config_dir = temp.path().join("python-rnsd-recipe-budget");
    let mut rnsd = None;
    let outcome = tokio::time::timeout(
        RECIPE_BUDGET_TOTAL,
        recipe_budget_scenario(started, &config_dir, &mut rnsd),
    )
    .await;
    roaming_expect_in_budget(outcome, &rnsd, "the recipe budget scenario");
}
