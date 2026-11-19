// ayeos-rpi / cli / src / capsules.rs — Pure-Rust scalar capsules published on https://g8t.is
use anyhow::ensure;
use anyhow::Result;

pub struct CapsuleMeta {
    pub name: &'static str,
    pub id: &'static str,
    pub signature: &'static str,
    pub cycles_per_op_pq: &'static str,
    pub foken_fkn: u64,
}

pub const PUBLISHED_CAPSULES: [CapsuleMeta; 3] = [
    CapsuleMeta {
        name: "ayeos_gate_0x62_fold_step",
        id: "2713856be5c490d5ad9bbeb30fea0ba4",
        signature: "(u64,u64,u64)->u64",
        cycles_per_op_pq: "1/1",
        foken_fkn: 500,
    },
    CapsuleMeta {
        name: "ayeos_bonobo_resonance_trit",
        id: "dd3c6603c13c499fa9ae953a547de802",
        signature: "(u64,u64)->i64",
        cycles_per_op_pq: "1/1",
        foken_fkn: 500,
    },
    CapsuleMeta {
        name: "ayeos_pxe_lease_ipv4_u32",
        id: "33bbdbe327de29807f87edf02cc09f5c",
        signature: "(u64,u64,u64,u64)->u64",
        cycles_per_op_pq: "1/1",
        foken_fkn: 500,
    },
];

pub fn ayeos_gate_0x62_fold_step(seed: u64, word: u64, lane_idx: u64) -> u64 {
    let shift = ((lane_idx & 7) << 3) as u32;
    let mixed = seed.rotate_left(7) ^ word.rotate_left(shift) ^ 0x6262_6262_6262_6262;
    (mixed & !0xFF) | 0x62
}

pub fn ayeos_bonobo_resonance_trit(raw_bytes: u64, packed_bytes: u64) -> i64 {
    if raw_bytes == 0 || packed_bytes == 0 || raw_bytes == packed_bytes {
        0
    } else if raw_bytes.saturating_mul(2) >= packed_bytes.saturating_mul(3) {
        1
    } else {
        -1
    }
}

pub fn ayeos_pxe_lease_ipv4_u32(
    subnet_u32: u64,
    mac_low24: u64,
    start_octet: u64,
    end_octet: u64,
) -> u64 {
    let lo = start_octet.min(254);
    let hi = end_octet.clamp(lo, 254);
    let span = (hi - lo) + 1;
    let folded = (mac_low24 ^ (mac_low24 >> 8) ^ (mac_low24 >> 16)) & 0xFFFF_FFFF;
    let host = lo + (folded % span);
    (subnet_u32 & 0xFFFF_FF00) | (host & 0xFF)
}

pub fn run_capsule_self_check() -> Result<()> {
    let sealed = ayeos_gate_0x62_fold_step(0x6262_6262_6262_6262, 0x0102_0304_0506_0708, 3);
    ensure!(sealed & 0xFF == 0x62, "0x62 gate invariant violated");

    ensure!(
        ayeos_bonobo_resonance_trit(797_184, 380_528) == 1,
        "Bonobo +1 resonance violated"
    );
    ensure!(
        ayeos_bonobo_resonance_trit(299, 200) == -1,
        "Bonobo -1 resonance violated"
    );
    ensure!(
        ayeos_bonobo_resonance_trit(200, 200) == 0,
        "Bonobo 0 resonance violated"
    );

    let subnet = 0x0A58_0000u64; // 10.88.0.0
    let ip = ayeos_pxe_lease_ipv4_u32(subnet, 0xAA_BBCC, 10, 250);
    let host = ip & 0xFF;
    ensure!((10..=250).contains(&host), "PXE host octet out of pool");
    ensure!(ip >> 8 == subnet >> 8, "PXE subnet prefix mismatch");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capsule_vectors_pass() {
        run_capsule_self_check().unwrap();
        assert_eq!(PUBLISHED_CAPSULES.len(), 3);
    }
}
