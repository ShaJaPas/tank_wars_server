/// Handle up migrations 
fn up(migr: &mut Migration) {
    migr.create_table("players", |t| {
        t.add_column("id", barrel::types::custom("BIGINT"));
    });
}

/// Handle down migrations 
fn down(migr: &mut Migration) {
    migr.drop_table("players");
} 
