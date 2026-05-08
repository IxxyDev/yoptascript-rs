pub const CLASS_TAG: &str = "__class__";
pub const SUPER: &str = "__super__";
pub const THIS: &str = "тырыпыры";

pub const ERROR_NAME: &str = "Косяк";
pub const ERROR_NAME_FIELD: &str = "name";
pub const ERROR_MESSAGE_FIELD: &str = "message";

pub const ITER_VALUE: &str = "значение";
pub const ITER_DONE: &str = "готово";

pub const DISPOSE_METHOD: &str = "расход";

pub const DEC_KIND: &str = "вид";
pub const DEC_NAME: &str = "имя";
pub const DEC_STATIC: &str = "статичное";
pub const DEC_PRIVATE: &str = "приватное";
pub const DEC_ADD_INITIALIZER: &str = "добавитьИнициализатор";

pub const GETTER_PREFIX: &str = "__get_";
pub const SETTER_PREFIX: &str = "__set_";
const ACCESSOR_SUFFIX: &str = "__";

#[must_use]
pub fn getter_key(prop: &str) -> String {
    format!("{GETTER_PREFIX}{prop}{ACCESSOR_SUFFIX}")
}

#[must_use]
pub fn setter_key(prop: &str) -> String {
    format!("{SETTER_PREFIX}{prop}{ACCESSOR_SUFFIX}")
}

#[must_use]
pub fn is_internal_key(k: &str) -> bool {
    k == CLASS_TAG || k.starts_with(GETTER_PREFIX) || k.starts_with(SETTER_PREFIX)
}
