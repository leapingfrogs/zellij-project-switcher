use std::collections::BTreeMap;

pub fn refresh_projects<RC>(config: Option<String>, mut f: RC)
where
    RC: FnMut(&[&str], BTreeMap<String, String>),
{
    let options = BTreeMap::from([(String::from("command"), String::from("refresh_projects"))]);

    let mut cmd: Vec<String> = Vec::from([
        String::from("fd"),
        String::from("-Htd"),
        String::from("--max-depth=2"),
        String::from("^\\.git$"),
    ]);

    let roots = config.unwrap_or(String::from("~"));
    for root in roots.split(':') {
        cmd.push(root.to_string());
    }

    let cmd: Vec<&str> = cmd.iter().map(String::as_ref).collect();
    f(&cmd, options);
}

#[cfg(test)]
mod test {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn refresh_projects_returns_expected_options() {
        let mut opts: Option<BTreeMap<String, String>> = None;

        refresh_projects(None, |_, context| -> () {
            opts = Some(context);
        });
        assert_eq!(
            opts,
            Some(BTreeMap::from([(
                String::from("command"),
                String::from("refresh_projects")
            )]))
        );
    }

    #[test]
    fn refresh_projects_base_command() {
        let mut cmd: Vec<String> = Vec::new();

        refresh_projects(None, |c, _| -> () {
            for item in c {
                cmd.push(item.to_string());
            }
        });
        assert_eq!(cmd[..4], vec!["fd", "-Htd", "--max-depth=2", "^\\.git$"]);
    }

    #[test]
    fn refresh_projects_with_default_root() {
        let mut cmd: Vec<String> = Vec::new();

        refresh_projects(None, |c, _| -> () {
            for item in c {
                cmd.push(item.to_string());
            }
        });
        assert_eq!(cmd[4..], vec!["~"]);
    }

    #[test]
    fn refresh_projects_with_configured_roots() {
        let mut cmd: Vec<String> = Vec::new();

        refresh_projects(
            Some(String::from("~/personal_projects:~/work_projects")),
            |c, _| -> () {
                for item in c {
                    cmd.push(item.to_string());
                }
            },
        );
        assert_eq!(cmd[4..], vec!["~/personal_projects", "~/work_projects"]);
    }
}
