use std::collections::HashMap;

pub struct Group {
    pub name: String,
    pub id: i32,
    pub mask: i32,
}

pub struct Table {
    pub groups: Vec<Group>,
    pub by_id: HashMap<i32, usize>,
}

pub fn parse(s: &str) -> Table {
    let mut groups: Vec<Group> = vec![];
    for entry in s.split('\\') {
        if entry.is_empty() {
            continue;
        }
        let parts: Vec<&str> = entry.split('^').collect();
        if parts.len() < 3 {
            continue;
        }
        let id: i32 = match parts[parts.len() - 2].trim().parse() {
            Ok(v) => v,
            Err(_) => continue,
        };
        let mask: i32 = parts[parts.len() - 1].trim().parse::<i64>().map(|m| m as i32).unwrap_or(-1);
        let name = parts[..parts.len() - 2].join("^");
        if !(0..32).contains(&id) {
            continue;
        }
        groups.push(Group { name, id, mask });
    }
    if !groups.iter().any(|g| g.id == 0) {
        groups.insert(0, Group { name: "Default".into(), id: 0, mask: -1 });
    }
    groups.sort_by_key(|g| g.id);
    let by_id = groups.iter().enumerate().map(|(i, g)| (g.id, i)).collect();
    Table { groups, by_id }
}

pub fn collide(t: &Table, a: i32, b: i32) -> bool {
    if a == b && a == 0 {
        return true;
    }
    let (ga, gb) = match (t.by_id.get(&a), t.by_id.get(&b)) {
        (Some(x), Some(y)) => (&t.groups[*x], &t.groups[*y]),
        _ => return true,
    };
    ((ga.mask as u32 >> (b as u32 & 31)) & 1) == 1 && ((gb.mask as u32 >> (a as u32 & 31)) & 1) == 1
}

pub fn rapier(t: &Table, id: i32) -> u32 {
    let bit = |n: i32| -> u32 { if n < 16 { 1u32 << n } else { 1u32 << 15 } };
    let mut filter = 0u32;
    for g in &t.groups {
        if collide(t, id, g.id) {
            filter |= bit(g.id);
        }
    }
    if id >= 16 || t.groups.iter().any(|g| g.id >= 16) {
        filter = 0xffff;
    }
    ((bit(id) & 0xffff) << 16) | (filter & 0xffff)
}
