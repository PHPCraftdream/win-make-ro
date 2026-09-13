use super::{Item, Selection};

/// Menu items for a selection, in display order.
pub fn plan(s: &Selection) -> Vec<Item> {
    let mut items = Vec::new();
    if s.any_unlocked {
        items.push(Item::MakeReadOnly);
    }
    if s.any_explicit {
        items.push(Item::RemoveReadOnly);
    }
    if items.is_empty() && s.any_inherited {
        items.push(Item::InheritedInfo);
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sel(u: bool, e: bool, i: bool) -> Selection {
        Selection { any_unlocked: u, any_explicit: e, any_inherited: i }
    }

    #[test]
    fn unlocked_only_offers_make() {
        assert_eq!(plan(&sel(true, false, false)), vec![Item::MakeReadOnly]);
    }

    #[test]
    fn explicit_only_offers_remove() {
        assert_eq!(plan(&sel(false, true, false)), vec![Item::RemoveReadOnly]);
    }

    #[test]
    fn mixed_offers_both_in_order() {
        assert_eq!(plan(&sel(true, true, true)), vec![Item::MakeReadOnly, Item::RemoveReadOnly]);
    }

    #[test]
    fn inherited_only_shows_disabled_info() {
        let items = plan(&sel(false, false, true));
        assert_eq!(items, vec![Item::InheritedInfo]);
        assert!(!items[0].enabled());
        assert_eq!(items[0].command(), None);
    }

    #[test]
    fn inherited_plus_unlocked_hides_info() {
        assert_eq!(plan(&sel(true, false, true)), vec![Item::MakeReadOnly]);
    }

    #[test]
    fn empty_selection_yields_nothing() {
        assert!(plan(&Selection::default()).is_empty());
    }
}
