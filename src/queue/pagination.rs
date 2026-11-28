//! Pure page arithmetic for bounded queue views.

/// Maximum number of queue rows rendered per page.
pub const PAGE_SIZE: usize = 10;

/// Number of pages needed for `total` rows; an empty queue still has one page.
pub fn page_count(total: usize) -> usize {
    if total == 0 {
        1
    } else {
        (total - 1) / PAGE_SIZE + 1
    }
}

/// Clamp a 1-based page number into `1..=pages`.
pub fn clamp_page(page: usize, pages: usize) -> usize {
    if pages == 0 {
        1
    } else {
        page.clamp(1, pages)
    }
}

/// Half-open row range for a 1-based page.
pub fn page_bounds(page: usize, total: usize) -> (usize, usize) {
    let page = clamp_page(page, page_count(total));
    let start = (page - 1) * PAGE_SIZE;
    (start, (start + PAGE_SIZE).min(total))
}

#[cfg(test)]
mod tests {
    use super::{clamp_page, page_bounds, page_count, PAGE_SIZE};

    #[test]
    fn counts_pages_at_boundaries() {
        assert_eq!(page_count(0), 1);
        assert_eq!(page_count(1), 1);
        assert_eq!(page_count(PAGE_SIZE), 1);
        assert_eq!(page_count(PAGE_SIZE + 1), 2);
        assert_eq!(page_count(25), 3);
    }

    #[test]
    fn clamps_page_numbers() {
        assert_eq!(clamp_page(0, 3), 1);
        assert_eq!(clamp_page(9, 3), 3);
        assert_eq!(clamp_page(2, 3), 2);
        assert_eq!(clamp_page(4, 0), 1);
    }

    #[test]
    fn computes_slice_bounds() {
        assert_eq!(page_bounds(1, 25), (0, 10));
        assert_eq!(page_bounds(2, 25), (10, 20));
        assert_eq!(page_bounds(3, 25), (20, 25));
        assert_eq!(page_bounds(9, 25), (20, 25));
        assert_eq!(page_bounds(1, 0), (0, 0));
    }
}
