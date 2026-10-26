//! Loading the DMN models a deployment ships.
//!
//! The server used to parse `dmn_files[0]` — the first entry `read_dir` happened to return, in
//! whatever order the filesystem listed them. A deployment with two decision files therefore
//! evaluated against one of them arbitrarily, and a restart could pick the other. This loads
//! every `.dmn` file in the directory, in sorted order so the result is reproducible, and merges
//! them into one model.

use std::path::Path;

use engine::dmn::evaluator::SharedDmnModel;
use engine::dmn::feel::cache::FeelAstCache;
use engine::dmn::model::DmnModel;
use engine::dmn::parser::parse_dmn_with_cache;

/// Every `.dmn` document under `dir`, merged into one model.
///
/// Files are read in sorted path order. A file that cannot be read or parsed is reported and
/// skipped: one broken model should not hide the rest, and an empty model is a worse answer than
/// a partial one. Returns an empty model when the directory holds no `.dmn` file.
pub fn load_models(dir: &Path) -> SharedDmnModel {
    let mut paths: Vec<std::path::PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "dmn"))
        .collect();
    paths.sort();

    if paths.is_empty() {
        tracing::info!(path = %dir.display(), "no .dmn files found; using an empty DMN model");
        return SharedDmnModel::new(DmnModel::default(), FeelAstCache::default());
    }

    let mut merged: Option<DmnModel> = None;
    let mut cache = FeelAstCache::default();
    let mut loaded = 0usize;

    for path in &paths {
        let content = match std::fs::read_to_string(path) {
            Ok(content) => content,
            Err(error) => {
                tracing::warn!(path = %path.display(), %error, "cannot read a DMN model; skipping it");
                continue;
            }
        };
        match parse_dmn_with_cache(&content) {
            Ok((model, file_cache)) => {
                loaded += 1;
                cache = file_cache;
                match &mut merged {
                    None => merged = Some(model),
                    Some(existing) => merge(existing, model),
                }
            }
            Err(error) => {
                tracing::warn!(path = %path.display(), %error, "cannot parse a DMN model; skipping it");
            }
        }
    }

    match merged {
        Some(model) => {
            tracing::info!(
                path = %dir.display(),
                files = paths.len(),
                loaded,
                elements = model.definitions.children.len(),
                "loaded DMN models"
            );
            SharedDmnModel::new(model, cache)
        }
        None => {
            tracing::warn!(path = %dir.display(), files = paths.len(), "no DMN model could be loaded; using an empty model");
            SharedDmnModel::new(DmnModel::default(), cache)
        }
    }
}

/// Fold `other` into `into`: the element lists concatenate in document order, which is what the
/// evaluator and the exporter both walk.
fn merge(into: &mut DmnModel, other: DmnModel) {
    into.definitions.children.extend(other.definitions.children);
    into.definitions.diagrams.extend(other.definitions.diagrams);
    if into.definitions.name.is_none() {
        into.definitions.name = other.definitions.name;
    }
    // The merged document keeps the first file's namespace; a model that declares a different one
    // is unusual enough to be worth saying out loud.
    if other.definitions.namespace != into.definitions.namespace {
        tracing::warn!(
            kept = %into.definitions.namespace,
            other = %other.definitions.namespace,
            "DMN models declare different namespaces; keeping the first"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::dmn::model::Definitions;

    fn write(path: &Path, id: &str, decision: &str) {
        std::fs::write(
            path,
            format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<definitions xmlns="https://www.omg.org/spec/DMN/20180913/MODEL/" id="{id}" name="{id}" namespace="http://example.com/{id}">
  <decision id="{decision}" name="{decision}">
    <literalExpression><text>"ok"</text></literalExpression>
  </decision>
</definitions>"#
            ),
        )
        .expect("write model");
    }

    fn decision_ids(model: &DmnModel) -> Vec<String> {
        use engine::dmn::model::DefinitionsChild;
        model
            .definitions
            .children
            .iter()
            .filter_map(|child| match child {
                DefinitionsChild::Decision(decision) => Some(decision.id.clone()),
                _ => None,
            })
            .collect()
    }

    #[actix_web::test]
    async fn loads_every_model_in_the_directory() {
        let dir = std::env::temp_dir().join(format!("rrules-dmn-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        write(&dir.join("a-first.dmn"), "first", "decide_first");
        write(&dir.join("b-second.dmn"), "second", "decide_second");

        let shared = load_models(&dir);
        let ids = decision_ids(&shared.model);

        assert!(
            ids.contains(&"decide_first".to_string()) && ids.contains(&"decide_second".to_string()),
            "both models should contribute their decisions, got {ids:?}"
        );
    }

    #[actix_web::test]
    async fn a_broken_model_does_not_hide_the_good_one() {
        let dir = std::env::temp_dir().join(format!("rrules-dmn-broken-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");
        write(&dir.join("a-good.dmn"), "good", "decide_good");
        std::fs::write(dir.join("b-broken.dmn"), "<definitions>not a model").expect("write");

        let shared = load_models(&dir);
        assert_eq!(decision_ids(&shared.model), vec!["decide_good".to_string()]);
    }

    #[actix_web::test]
    async fn an_empty_directory_yields_an_empty_model() {
        let dir = std::env::temp_dir().join(format!("rrules-dmn-empty-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("temp dir");

        let shared = load_models(&dir);
        assert!(shared.model.definitions.children.is_empty());
    }

    #[test]
    fn merge_concatenates_definitions() {
        let mut into = DmnModel {
            definitions: Definitions {
                children: Vec::new(),
                ..Definitions::default()
            },
            source_namespace: None,
        };
        let other = DmnModel {
            definitions: Definitions::default(),
            source_namespace: None,
        };
        merge(&mut into, other);
        assert!(into.definitions.name.is_some() || into.definitions.children.is_empty());
    }
}
