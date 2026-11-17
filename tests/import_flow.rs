use laburo::db::Db;
use laburo::import::import_casos;
use laburo::repo;
use std::path::Path;

#[test]
fn imports_a_class_dictionary_and_links_the_use_case() {
    let db = Db::open_memory().unwrap();
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/casos_small.yaml");
    let count = import_casos(&db.conn, &path).unwrap();
    assert_eq!(count, 1);

    let case = repo::get_case(&db.conn, "TC-M02-01-1").unwrap().unwrap();
    assert_eq!(case.title, "La reserva persiste");
    assert_eq!(case.steps.len(), 2);
    assert!(case.tags.iter().any(|t| t == "unit"));
    assert!(
        case.tags
            .iter()
            .any(|t| t == "funcional" || t == "Funcional")
    );

    let requirements: i64 = db
        .conn
        .query_row("SELECT count(*) FROM requirement", [], |r| r.get(0))
        .unwrap();
    assert_eq!(requirements, 1);
    let links: i64 = db
        .conn
        .query_row("SELECT count(*) FROM case_requirement", [], |r| r.get(0))
        .unwrap();
    assert_eq!(links, 1);

    // Idempotent: a second import does not duplicate the case.
    let again = import_casos(&db.conn, &path).unwrap();
    assert_eq!(again, 0);
    let cases: i64 = db
        .conn
        .query_row("SELECT count(*) FROM test_case", [], |r| r.get(0))
        .unwrap();
    assert_eq!(cases, 1);
}
