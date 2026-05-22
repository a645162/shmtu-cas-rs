pub mod bill;
pub mod export;
pub mod hot_water;

// 向后兼容旧接口：直接 use shmtu_cas::parser::{BillItem, parse_bill_list, get_total_pages}
pub use bill::{BillParseResult, parse_bill_item, parse_bill_list, parse_bill_page, get_total_pages};
pub use crate::datatype::bill::BillItem;
