/*
 * This file is for infoce insha'Allah
 * the rools of configs and user inputs
 * The goal is insure we don't have insha'Allah
 * wrong or unvalid data.
 */

use std::collections::VecDeque;

use miette::miette;

use crate::pkg::Input;

pub fn ensure_no_duplication(input: &Input) -> Option<String> {
    for (index, this) in input.iter().enumerate() {
        for other in input.iter().skip(index + 1) {
            if this.name == other.name {
                return Some(this.name.clone());
            }
        }
    }

    None
}

pub fn ensure_all_depand_on_defined_pkg(input: &Input) -> Option<String> {
    for pkg in input {
        if let Some(dep) = pkg.deps.clone().and_then(|deps| {
            deps.iter()
                .find(|dep| !input.iter().any(|pkg| pkg.name == **dep))
                .cloned()
        }) {
            return Some(dep.clone());
        }
    }

    None
}

pub fn try_ensure_no_dep_loop(input: &Input) -> miette::Result<Option<Vec<String>>> {
    for one in input {
        let mut deps_eque = VecDeque::from([one.name.clone()]);
        let mut deps_stuck = Vec::new();

        while let Some(dep) = deps_eque.pop_front() {
            if deps_stuck.contains(&dep) {
                deps_stuck.push(dep.clone());
                return Ok(Some(deps_stuck));
            }

            deps_stuck.push(dep.clone());

            let pkg = input.iter().find(|pkg| pkg.name == dep);

            if pkg.is_none() {
                return Err(miette!(format!(
                    "pkg {} depand on pkg not exists: {dep}",
                    one.name
                )));
            }

            if let Some(other_deps) = pkg.unwrap().deps.clone() {
                deps_eque.append(&mut other_deps.into());
            }
        }
    }

    Ok(None)
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::pkg::{PkgOption, PkgUserDef};

    #[test]
    fn input_validations_positive() -> miette::Result<()> {
        let input = vec![
            PkgUserDef {
                name: "pkg1".into(),
                input: "pkg1".into(),
                opts: vec![PkgOption::Bool {
                    name: "are-u-okey".into(),
                    value: true,
                }]
                .into(),
                deps: vec!["pkg2".into()].into(),
                version: None,
                just_a_dep: None,
                os: None,
            },
            PkgUserDef {
                name: "pkg2".into(),
                input: "pkg2".into(),
                opts: vec![PkgOption::Bool {
                    name: "are-u-okey".into(),
                    value: true,
                }]
                .into(),
                deps: vec![].into(),
                version: None,
                just_a_dep: None,
                os: None,
            },
        ];

        let dupl = ensure_no_duplication(&input);
        let unknow_dep = ensure_all_depand_on_defined_pkg(&input);
        let dep_loop = try_ensure_no_dep_loop(&input)?;

        assert_eq!(dupl, None);
        assert_eq!(unknow_dep, None);
        assert_eq!(dep_loop, None);

        Ok(())
    }

    #[test]
    fn input_validations_nigative() -> miette::Result<()> {
        let input = vec![
            PkgUserDef {
                name: "pkg1".into(),
                input: "pkg1".into(),
                opts: vec![PkgOption::Bool {
                    name: "are-u-okey".into(),
                    value: true,
                }]
                .into(),
                deps: vec!["pkg2".into()].into(),
                version: None,
                just_a_dep: None,
                os: None,
            },
            PkgUserDef {
                name: "pkg1".into(),
                input: "what_ever".into(),
                opts: vec![].into(),
                deps: vec![].into(),
                version: None,
                just_a_dep: None,
                os: None,
            },
            PkgUserDef {
                name: "bad_man".into(),
                input: "what_ever".into(),
                opts: vec![].into(),
                deps: vec!["not_exists".into()].into(),
                version: None,
                just_a_dep: None,
                os: None,
            },
            PkgUserDef {
                name: "pkg2".into(),
                input: "pkg2".into(),
                opts: vec![PkgOption::Bool {
                    name: "are-u-okey".into(),
                    value: true,
                }]
                .into(),
                deps: vec![].into(),
                version: None,
                just_a_dep: None,
                os: None,
            },
        ];

        let depand_on_my_self = vec![PkgUserDef {
            name: "bad_man".into(),
            input: "bad_input".into(),
            opts: vec![].into(),
            deps: vec!["bad_man".into()].into(),
            version: None,
            just_a_dep: None,
            os: None,
        }];

        let depand_on_someone_and_he_depand_on_me = vec![
            PkgUserDef {
                name: "bad_man1".into(),
                input: "bad_input1".into(),
                opts: vec![].into(),
                deps: vec!["bad_man2".into()].into(),
                version: None,
                just_a_dep: None,
                os: None,
            },
            PkgUserDef {
                name: "bad_man2".into(),
                input: "bad_input2".into(),
                opts: vec![].into(),
                deps: vec!["bad_man1".into()].into(),
                version: None,
                just_a_dep: None,
                os: None,
            },
        ];

        let depand_on_someone_depand_on_someone_depand_on_me = vec![
            PkgUserDef {
                name: "bad_man1".into(),
                input: "bad_input1".into(),
                opts: vec![].into(),
                deps: vec!["bad_man3".into()].into(),
                version: None,
                just_a_dep: None,
                os: None,
            },
            PkgUserDef {
                name: "bad_man2".into(),
                input: "bad_input2".into(),
                opts: vec![].into(),
                deps: vec!["bad_man1".into()].into(),
                version: None,
                just_a_dep: None,
                os: None,
            },
            PkgUserDef {
                name: "bad_man3".into(),
                input: "bad_input3".into(),
                opts: vec![].into(),
                deps: vec!["bad_man2".into()].into(),
                version: None,
                just_a_dep: None,
                os: None,
            },
        ];

        let dupl = ensure_no_duplication(&input);
        let unknow_dep = ensure_all_depand_on_defined_pkg(&input);
        let dep_loop = try_ensure_no_dep_loop(&input);

        assert_eq!(dupl, Some("pkg1".into()));
        assert_eq!(unknow_dep, Some("not_exists".into()));
        assert!(dep_loop.is_err());

        let dont_depand_on_your_self = try_ensure_no_dep_loop(&depand_on_my_self)?;
        let dont_depand_on_someone_and_he_depand_on_you =
            try_ensure_no_dep_loop(&depand_on_someone_and_he_depand_on_me)?;
        let dont_do_that_what_ever =
            try_ensure_no_dep_loop(&depand_on_someone_depand_on_someone_depand_on_me)?;

        assert_eq!(
            dont_depand_on_your_self,
            Some(vec!["bad_man".into(), "bad_man".into()])
        );

        assert_eq!(
            dont_depand_on_someone_and_he_depand_on_you,
            Some(vec![
                "bad_man1".into(),
                "bad_man2".into(),
                "bad_man1".into()
            ])
        );

        assert_eq!(
            dont_do_that_what_ever,
            Some(vec![
                "bad_man1".into(),
                "bad_man3".into(),
                "bad_man2".into(),
                "bad_man1".into(),
            ])
        );

        Ok(())
    }
}
