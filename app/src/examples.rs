//! The example apps that come with Blockwise. Each one's project and files
//! are built into the program from examples/<key>/; the Examples page (and a
//! first start) adds a copy to the projects. examples/export.py refreshes
//! them from a project made in Blockwise.

use crate::model::{now_secs, Project, Store};

pub struct Example {
    pub key: &'static str,
    pub title: &'static str,
    pub blurb: &'static str,
    /// What it shows, one short line each.
    pub shows: &'static [&'static str],
    project: &'static str,
    files: &'static [(&'static str, &'static [u8])],
    /// A phone screenshot for its card.
    pub thumb: &'static [u8],
}

pub const EXAMPLES: &[Example] = &[
    Example {
        key: "component-tour",
        title: "Component Tour",
        blurb: "Every native widget in one app: buttons, text fields, switches, a slider, a dropdown, a pop-up, a bottom sheet, a calendar and a list.",
        shows: &["App bar buttons", "Bottom sheet", "Pop-up dialog", "Calendar and list"],
        project: include_str!("../../examples/component-tour/project.json"),
        files: &[],
        thumb: include_bytes!("../../examples/component-tour/screenshots/thumb.png"),
    },
    Example {
        key: "brain-quiz",
        title: "Brain Quiz",
        blurb: "Ten random questions from a JSON file, a 15-second timer, green and red answers, and a best score saved on the phone.",
        shows: &["Three screens", "Timer and progress bar", "Data file", "Saved best score"],
        project: include_str!("../../examples/brain-quiz/project.json"),
        files: &[("questions.json", include_bytes!("../../examples/brain-quiz/assets/questions.json"))],
        thumb: include_bytes!("../../examples/brain-quiz/screenshots/thumb.png"),
    },
];

impl Example {
    pub fn project(&self) -> Project {
        serde_json::from_str(self.project).unwrap_or_else(|e| panic!("example {} doesn't load: {e}", self.key))
    }
}

/// Adds a copy of the example to the projects, with its files. Returns the
/// new project's id.
pub fn add(store: &mut Store, ex: &Example) -> Result<u32, String> {
    let mut p = ex.project();
    p.id = store.next_project;
    let base = p.name.clone();
    let mut k = 1;
    while store.projects.iter().any(|q| q.name == p.name) {
        k += 1;
        p.name = format!("{base} {k}");
    }
    if k > 1 {
        p.app_name = format!("{} {k}", p.app_name);
    }
    p.modified = now_secs();
    // blocks made from now on must not take the example's block ids
    crate::model::bump_ids(p.screens.iter().map(|s| s.max_block_id()).max().unwrap_or(0));
    if !ex.files.is_empty() {
        let dir = p.files_dir();
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make the files folder: {e}"))?;
        for (name, bytes) in ex.files {
            std::fs::write(dir.join(name), bytes).map_err(|e| format!("Couldn't copy {name}: {e}"))?;
        }
    }
    store.next_project += 1;
    let id = p.id;
    store.projects.push(p);
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn examples_load_and_add_with_their_files() {
        let mut st = Store { next_project: 900, ..Default::default() };
        for ex in EXAMPLES {
            let p = ex.project();
            assert!(!p.screens.is_empty(), "{}", ex.key);
            // every file the project lists is built in
            for f in &p.files {
                assert!(ex.files.iter().any(|(n, _)| *n == f.file_name()), "{} misses {}", ex.key, f.file_name());
            }
            assert!(image::load_from_memory(ex.thumb).is_ok(), "{} thumbnail", ex.key);
            let id = add(&mut st, ex).unwrap();
            let q = st.projects.iter().find(|q| q.id == id).unwrap();
            for f in &q.files {
                assert!(q.file_path(f).exists(), "{} file {} not copied", ex.key, f.file_name());
            }
            // the generated code builds from it
            for s in &q.screens {
                crate::native::kotlin(q, s);
                crate::native::swift_screen(q, s);
            }
        }
        // a second copy gets its own name and id
        let again = add(&mut st, &EXAMPLES[0]).unwrap();
        let names: Vec<&str> = st.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names.len(), EXAMPLES.len() + 1);
        assert!(names.contains(&format!("{} 2", EXAMPLES[0].project().name).as_str()));
        assert_eq!(st.next_project, 900 + EXAMPLES.len() as u32 + 1);
        for p in &st.projects {
            let _ = std::fs::remove_dir_all(p.files_dir());
        }
        let _ = again;
    }
}
