//! Styles shared by the library and app pages.

pub mod app;
pub mod library;

use zgui::style;

style! { pub PageStyle => r#"
    /* Share the inset between the cards and search field so their edges line up. */
    :scope {
        position: relative;
        flex: 1 1 auto;
        min-height: 0;
        height: 100%;
        --page-inset: max(var(--zui-space-xl), calc((100% - 760px) / 2 + var(--zui-space-xl)));
    }

    :scope .scroll {
        flex: 1 1 auto;
        min-height: 0;
    }

    /* Leave enough bottom padding to scroll the last card clear of the floating button. */
    :scope .add-button {
        position: absolute;
        right: var(--zui-space-lg);
        bottom: var(--zui-space-lg);
    }

    :scope.fab .body {
        padding-bottom: calc(44px + var(--zui-space-lg) * 2);
    }

    :scope .body {
        gap: var(--zui-space-2xl);
        padding: var(--zui-space-xl) var(--page-inset);
        width: 100%;
        /* at least the page's height, so an empty state can center in it */
        min-height: 100%;
    }

    :scope .search {
        flex: 0 0 auto;
        /* zgui can't interpolate auto heights. Use the field's 36px height plus padding and
           border so the search can animate. */
        height: 53px;
        align-items: center;
        padding: 0 var(--page-inset);
        border-bottom: 1px solid var(--zui-color-border);
        overflow: hidden;

        animation: search-open var(--zui-motion-duration-slower) cubic-bezier(0.32, 0.72, 0, 1);
    }

    :scope .search[data-state="closed"] {

        animation: search-close var(--zui-motion-duration-slow) var(--zui-motion-ease-standard) forwards;
        pointer-events: none;
    }

    @keyframes search-open {
        from {
            height: 0px;
            opacity: 0;
        }
    }

    @keyframes search-close {
        to {
            height: 0px;
            opacity: 0;
        }
    }

    :scope .list {
        gap: var(--zui-space-sm);
    }

    :scope .card {
        background-color: var(--zui-color-card);
    }

    :scope .card.zui-collapsible {
        border: 1px solid var(--zui-color-border);
        border-radius: var(--zui-radius-md);
        overflow: hidden;
    }

    :scope .heading {
        border: none;
        border-radius: 0;
    }

    :scope .expander {
        width: 30px;
        height: 30px;
        padding: 0;
        justify-content: center;
        color: var(--zui-color-muted-foreground);
        transition-property: background-color, color;
        transition-duration: var(--zui-motion-duration-fast);
    }

    :scope .expander:hover {
        background-color: var(--zui-color-accent);
        color: var(--zui-color-accent-foreground);
    }

    :scope .chevron {
        transition: transform var(--zui-motion-duration-slow) var(--zui-motion-ease-standard);
    }

    :scope .chevron.open {
        transform: rotate(180deg);
    }

    /* Pad the rows individually so the separators can span the full card width. */
    :scope .settings {
        border-top: 1px solid var(--zui-color-border);
    }

    :scope .settings .zui-settings__item {
        padding: var(--zui-space-lg);
    }
"# }
