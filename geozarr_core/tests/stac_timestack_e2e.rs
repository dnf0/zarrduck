use geozarr_core::dataset::ZarrDataset;
use geozarr_core::query_planner::QueryConstraints;
use std::collections::HashMap;

fn fixt(name: &str) -> String {
    format!("{}/tests/fixtures/{}", env!("CARGO_MANIFEST_DIR"), name)
}
fn allow() {
    std::env::set_var("GEOZARR_ALLOW_PATH", env!("CARGO_MANIFEST_DIR"));
}

#[test]
fn timestack_opens_as_3d_with_time_coords() {
    allow();
    let ds = ZarrDataset::open_with_asset(&fixt("stac_itemcollection.json"), Some("band")).unwrap();
    assert_eq!(
        ds.dim_names,
        vec!["time".to_string(), "lat".to_string(), "lon".to_string()]
    );
    assert_eq!(ds.shape, vec![2u64, 2, 4]);
    let time = ds.coords.get("time").expect("time coords present");
    // 2026-01-01 and 2026-02-01 in epoch seconds, ascending
    assert_eq!(time.len(), 2);
    assert!(time[0] < time[1]);
    assert_eq!(time[0], 1767225600.0);
    // value dtype is Int16
    let (vname, vtype) = ds.schema().unwrap().pop().unwrap();
    assert_eq!(vname, "value");
    assert_eq!(
        format!("{vtype:?}"),
        format!("{:?}", zarrs::array::DataType::Int16)
    );
}

#[test]
fn timestack_time_pushdown_prunes_to_one_slice() {
    allow();
    let ds = ZarrDataset::open_with_asset(&fixt("stac_itemcollection.json"), Some("band")).unwrap();
    // bracket only the first datetime
    let mut bounds = HashMap::new();
    bounds.insert(
        "time".to_string(),
        (Some(1767225600.0 - 10.0), Some(1767225600.0 + 10.0)),
    );
    let constraints = QueryConstraints {
        bounds,
        pins: HashMap::new(),
    };
    let (bmin, bmax) = ds.compute_bounds(&constraints);
    assert_eq!(
        (bmin[0], bmax[0]),
        (0, 0),
        "time should prune to index 0 only"
    );
}
