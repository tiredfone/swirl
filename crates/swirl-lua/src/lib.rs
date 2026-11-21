use mlua::Lua;

pub fn new_lua() -> Lua {
    Lua::new()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lua51_semantics() {
        let lua = new_lua();
        let v: String = lua.load("return _VERSION").eval().unwrap();
        assert_eq!(v, "Lua 5.1");
        let n: i64 = lua.load("local t = {} for i=1,5 do t[#t+1]=i end return #t").eval().unwrap();
        assert_eq!(n, 5);
        let ok: bool = lua.load("return string.split == nil").eval().unwrap();
        assert!(ok);
        let g: bool = lua.load("return setfenv ~= nil and unpack ~= nil").eval().unwrap();
        assert!(g);
    }
}
