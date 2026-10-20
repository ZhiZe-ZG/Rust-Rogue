use clap::Parser;

#[derive(Parser)]
#[command(name = "rogue", version)]
pub struct CommandLineParameter {
    #[arg(long = "seed", value_name = "SEED")]
    pub(crate) seed: Option<i32>,
    #[arg(short = 'r', long = "restore", num_args = 0..=1, default_missing_value = "-r", conflicts_with_all = ["save_file"], value_name = "FILE")]
    pub(crate) restore: Option<String>,
    #[arg(value_name = "SAVE_FILE", allow_hyphen_values = true, conflicts_with_all = ["restore"])]
    pub(crate) save_file: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::CommandLineParameter;
    use clap::Parser;

    #[test]
    fn parses_startup_arguments() {
        assert_eq!(
            CommandLineParameter::try_parse_from(["rogue", "--seed", "12345"])
                .unwrap()
                .seed,
            Some(12345)
        );

        let default_restore = CommandLineParameter::try_parse_from(["rogue", "-r"]).unwrap();
        assert_eq!(default_restore.restore.as_deref(), Some("-r"));

        let explicit_restore =
            CommandLineParameter::try_parse_from(["rogue", "--restore", "save.dat"]).unwrap();
        assert_eq!(explicit_restore.restore.as_deref(), Some("save.dat"));

        let positional_restore =
            CommandLineParameter::try_parse_from(["rogue", "save.dat"]).unwrap();
        assert_eq!(positional_restore.save_file.as_deref(), Some("save.dat"));

        let wizard_argument = CommandLineParameter::try_parse_from(["rogue", ""]).unwrap();
        assert_eq!(wizard_argument.save_file.as_deref(), Some(""));
    }
}
