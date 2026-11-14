//! The example apps that come with Blockwise. Each one is built into the
//! program from its .bw file in examples/ (the same file people download);
//! the Examples page (and a first start) adds a copy to the projects.
//! app/export_example.py refreshes them from a project made in Blockwise.

use crate::model::{read_project_bytes, Imported, Project, Store};

pub struct Example {
    pub key: &'static str,
    pub title: &'static str,
    pub blurb: &'static str,
    /// What it shows, one short line each.
    pub shows: &'static [&'static str],
    /// The .bw project file.
    pub bw: &'static [u8],
    /// A phone screenshot for its card.
    pub thumb: &'static [u8],
}

pub const EXAMPLES: &[Example] = &[
    Example {
        key: "component-tour",
        title: "Component Tour",
        blurb: "Every native widget in one app: buttons, text fields, switches, a slider, a dropdown, a pop-up, a bottom sheet, a calendar and a list.",
        shows: &["App bar buttons", "Bottom sheet", "Pop-up dialog", "Calendar and list"],
        bw: include_bytes!("../../examples/component-tour.bw"),
        thumb: include_bytes!("../../docs/images/examples/component-tour-card.png"),
    },
    Example {
        key: "brain-quiz",
        title: "Brain Quiz",
        blurb: "Ten random questions from a JSON file, a 15-second timer, green and red answers, and a best score saved on the phone.",
        shows: &["Three screens", "Timer and progress bar", "Data file", "Saved best score"],
        bw: include_bytes!("../../examples/brain-quiz.bw"),
        thumb: include_bytes!("../../docs/images/examples/brain-quiz-card.png"),
    },
];

impl Example {
    fn read(&self) -> Imported {
        read_project_bytes(self.bw, None).unwrap_or_else(|e| panic!("example {} doesn't load: {e}", self.key))
    }
    pub fn project(&self) -> Project {
        self.read().project
    }
}

/// Adds a copy of the example to the projects, with its files. Returns the
/// new project's id.
pub fn add(store: &mut Store, ex: &Example) -> Result<u32, String> {
    let im = ex.read();
    store.add_copy(im.project, im.files)
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
            // every file the project lists is in its .bw
            assert!(ex.read().missing.is_empty(), "{} misses {:?}", ex.key, ex.read().missing);
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
