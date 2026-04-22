import { ReactNode, useState } from "react";

interface CollapsiblePanelProps {
  eyebrow?: string;
  title?: string;
  children: ReactNode;
  actions?: ReactNode;
  defaultCollapsed?: boolean;
}

export default function CollapsiblePanel({
  eyebrow,
  title,
  children,
  actions,
  defaultCollapsed = false
}: CollapsiblePanelProps) {
  const [collapsed, setCollapsed] = useState(defaultCollapsed);

  return (
    <section className="panel collapsible-panel">
      <div className="collapsible-header">
        <button
          className={`collapsible-corner-toggle ${collapsed ? "collapsed" : ""}`}
          type="button"
          onClick={() => setCollapsed((value) => !value)}
          aria-label={collapsed ? "Expand section" : "Collapse section"}
        >
          <svg
            className="collapsible-corner-icon"
            viewBox="0 0 24 24"
            aria-hidden="true"
          >
            <path
              d="M7 10l5 5 5-5"
              fill="none"
              stroke="currentColor"
              strokeWidth="2.4"
              strokeLinecap="round"
              strokeLinejoin="round"
            />
          </svg>
        </button>

        <div className="collapsible-header-main">
          <button
            className="collapsible-toggle"
            type="button"
            onClick={() => setCollapsed((value) => !value)}
          >
            <div>
              {eyebrow ? <h2 className="panel-title">{eyebrow}</h2> : null}
              {title ? <p className="panel-subtitle">{title}</p> : null}
            </div>
          </button>

          {actions ? <div className="collapsible-actions">{actions}</div> : null}
        </div>
      </div>

      {!collapsed ? <div className="collapsible-body">{children}</div> : null}
    </section>
  );
}