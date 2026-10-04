#[cfg(test)]
mod tests {
    use super::*;

    const MEMINFO: &str = "MemTotal:       16000000 kB\nMemFree:  100 kB\nMemAvailable:    1000000 kB\nSwapTotal:       8000000 kB\nSwapFree:        2000000 kB\n";

    #[test]
    fn parses_meminfo() {
        let m = parse_meminfo(MEMINFO);
        assert_eq!(
            (m.total_kb, m.available_kb, m.swap_total_kb, m.swap_free_kb),
            (16_000_000, 1_000_000, 8_000_000, 2_000_000)
        );
    }

    fn sample() -> Sample {
        Sample {
            disks: vec![Disk { mount: "/".into(), total: 100, free: 50 }],
            mem: Some(parse_meminfo(MEMINFO)),
            cpu_temp_c: Some(50.0),
            nvme: vec![Nvme {
                name: "KINGSTON".into(),
                critical_warnings: vec![],
                temp_c: Some(37.0),
                percent_used: Some(4),
            }],
        }
    }

    fn level(checks: &[Check], id: &str) -> Level {
        checks.iter().find(|c| c.id == id).map(|c| c.level).unwrap()
    }

    #[test]
    fn thresholds() {
        let c = evaluate(&sample());
        assert_eq!(level(&c, "disk:/"), Level::Ok);
        assert_eq!(level(&c, "memory"), Level::Warn); // 6.25 % available
        assert_eq!(level(&c, "swap"), Level::Warn); // 75 % used
        assert_eq!(level(&c, "cpu"), Level::Ok);
        assert_eq!(level(&c, "nvme:KINGSTON"), Level::Ok);

        let mut s = sample();
        s.disks[0].free = 5;
        s.cpu_temp_c = Some(95.0);
        s.nvme[0].critical_warnings = vec!["spare".into()];
        let c = evaluate(&s);
        assert_eq!(level(&c, "disk:/"), Level::Bad);
        assert_eq!(level(&c, "cpu"), Level::Bad);
        assert_eq!(level(&c, "nvme:KINGSTON"), Level::Bad);
    }

    #[test]
    fn missing_sources_are_absent_not_errors() {
        let s = Sample { disks: vec![], mem: None, cpu_temp_c: None, nvme: vec![] };
        assert!(evaluate(&s).is_empty());
    }

    #[test]
    fn worst_level() {
        assert_eq!(worst(&evaluate(&sample())), Level::Warn);
        assert_eq!(worst(&[]), Level::Ok);
    }
}
