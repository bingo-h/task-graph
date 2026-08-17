//! 系统字体枚举
//!
//! 用纯 Rust 的 fontdb 扫描系统字体目录（不依赖 fontconfig/DirectWrite/CoreText
//! 这类平台原生 API 绑定，跨平台编译更省心，在打包环境缺依赖的机器上也不会像
//! 系统颜色选择器那样直接崩溃退出），供设置里"字体"下拉框做模糊搜索用。

use std::collections::BTreeSet;

/// 返回系统已安装字体的家族名列表，按字母序去重排列
pub fn list_families() -> Vec<String> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();

    let mut families: BTreeSet<String> = BTreeSet::new();
    for face in db.faces() {
        if let Some((family, _lang)) = face.families.first() {
            families.insert(family.clone());
        }
    }

    families.into_iter().collect()
}
