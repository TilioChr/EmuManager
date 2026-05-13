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
          <span className="collapsible-corner-icon" aria-hidden="true" />
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
