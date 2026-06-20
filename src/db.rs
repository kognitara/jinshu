use crate::store::GraphStore;

pub fn get_all_databases() -> Vec<String> {
    let d = GraphStore::new().list_databases();
    if d.is_ok() {
        d.expect("failed to get databases")
    } else {
        Vec::new()
    }
}
