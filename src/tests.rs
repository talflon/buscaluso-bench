use super::*;

#[test]
fn test_runcfg_deserialize() {
    assert_eq!(
        toml::from_str(
            r#"
            repeat = 7
            timeout = 8.3
            verbose = 1
            "#
        ),
        Ok(BenchRunCfg {
            repeat: 7,
            timeout: Duration::from_secs_f64(8.3),
            verbose: 1,
            rules_file: None,
            dict_file: None,
            bench_file: None,
        })
    );
}

#[test]
fn test_runcfg_deserialize_verbose_default() -> Result<(), toml::de::Error> {
    let cfg: BenchRunCfg = toml::from_str(
        r#"
        repeat = 7
        timeout = 8.3
        "#,
    )?;
    assert_eq!(cfg.verbose, 0);
    Ok(())
}

#[test]
fn test_runcfg_serialize_deserialize() -> Result<(), toml::ser::Error> {
    let cfg = BenchRunCfg {
        repeat: 20,
        verbose: 5,
        timeout: Duration::from_secs_f64(2.5),
        rules_file: None,
        dict_file: None,
        bench_file: None,
    };
    assert_eq!(toml::from_str(&toml::to_string(&cfg)?), Ok(cfg));
    Ok(())
}
