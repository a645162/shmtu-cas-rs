use anyhow::Result;
use std::collections::HashSet;
use crate::cas::epay::EpayAuth;
use crate::datatype::bill::{BillItem, BillType};

pub type SyncPageProgressCallback = dyn Fn(SyncPageProgress) + Send + Sync;

/// 调用方提供的数据存储抽象。库不关心底层是 JSON / SQLite / 内存。
pub trait BillStore: Send + Sync {
    /// 判断某条交易号是否已存在于本地
    fn contains(&self, number: &str) -> bool;

    /// 将新增条目合并到本地（调用方自行决定持久化策略）
    fn merge(&mut self, new_bills: Vec<BillItem>);
}

#[derive(Debug, Clone)]
pub struct SyncOptions {
    /// 从第几页开始（默认 1）
    pub start_page: u32,
    /// 最大翻页数（防止无限翻页）
    pub max_pages: u32,
    /// 账单类型
    pub bill_type: BillType,
    /// 连续遇到多少条已存在的交易号就早停
    pub early_stop_threshold: u32,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            start_page: 1,
            max_pages: 100,
            bill_type: BillType::All,
            early_stop_threshold: 5,
        }
    }
}

#[derive(Debug)]
pub struct SyncResult {
    /// 本次新增的条目数
    pub new_count: usize,
    /// 翻了多少页
    pub pages_fetched: u32,
    /// 是否因早停条件而终止
    pub early_stopped: bool,
    /// 所有新增条目
    pub new_bills: Vec<BillItem>,
}

#[derive(Debug, Clone)]
pub struct SyncPageProgress {
    /// 当前已拉取到的页面编号
    pub page: u32,
    /// 账单总页数
    pub total_pages: u32,
    /// 当前账号截至本页累计发现的新账单数
    pub new_count: usize,
}

/// 增量同步：逐页拉取账单，用交易号去重，遇到连续 N 条已知条目则早停。
pub async fn incremental_sync(
    epay: &EpayAuth,
    store: &mut dyn BillStore,
    options: &SyncOptions,
) -> Result<SyncResult> {
    incremental_sync_with_progress::<SyncPageProgressCallback>(epay, store, options, None).await
}

/// 带页级进度回调的增量同步。
pub async fn incremental_sync_with_progress<F>(
    epay: &EpayAuth,
    store: &mut dyn BillStore,
    options: &SyncOptions,
    progress_callback: Option<&F>,
) -> Result<SyncResult>
where
    F: Fn(SyncPageProgress) + Send + Sync + ?Sized,
{
    let tab_no = options.bill_type.tab_no();
    let mut new_bills = Vec::new();
    let mut pages_fetched = 0u32;
    let mut consecutive_known = 0u32;
    let mut early_stopped = false;
    let mut seen_numbers = HashSet::new();

    for page_offset in 0..options.max_pages {
        let page_no = options.start_page + page_offset;
        let html = epay.get_bill(page_no, tab_no).await?;
        let page_result = crate::parser::parse_bill_page(&html)?;

        if page_result.bills.is_empty() && page_offset == 0 {
            break;
        }

        pages_fetched += 1;

        for bill in page_result.bills {
            let is_known = !bill.number.is_empty()
                && (store.contains(&bill.number) || !seen_numbers.insert(bill.number.clone()));
            if is_known {
                consecutive_known += 1;
                if consecutive_known >= options.early_stop_threshold {
                    early_stopped = true;
                    break;
                }
            } else {
                consecutive_known = 0;
                new_bills.push(bill);
            }
        }

        if let Some(cb) = progress_callback {
            cb(SyncPageProgress {
                page: page_no,
                total_pages: page_result.total_pages.max(page_no),
                new_count: new_bills.len(),
            });
        }

        if early_stopped {
            break;
        }

        // 已到最后一页
        if page_no >= page_result.total_pages {
            break;
        }
    }

    let new_count = new_bills.len();
    store.merge(new_bills.clone());

    Ok(SyncResult {
        new_count,
        pages_fetched,
        early_stopped,
        new_bills,
    })
}
