import { useState, type ReactNode } from "react";

interface CollapsiblePanelProps {
  eyebrow?: string;
  title: string;
  defaultCollapsed?: boolean;
  children: ReactNode;
  actions?: ReactNode;
}

export default function CollapsiblePanel({
  eyebrow,
  title,
  defaultCollapsed = false,
  children,
  actions
}: CollapsiblePanelProps) {
  const [collapsed, setCollapsed] = useState(defaultCollapsed);

  return (
    <section className={`panel collapsible-panel ${collapsed ? "panel-collapsed" : ""}`}>
      <div className="collapsible-header">
        <button
          type="button"
          className="collapsible-toggle"
          onClick={() => setCollapsed((current) => !current)}
        >
          <div>
            {eyebrow && <p className="eyebrow">{eyebrow}</p>}
            <h2>{title}</h2>
          </div>
          <span className={`collapsible-chevron ${collapsed ? "collapsed" : ""}`}>⌄</span>
        </button>

        {actions && !collapsed && <div className="collapsible-actions">{actions}</div>}
      </div>

      {!collapsed && <div className="collapsible-body">{children}</div>}
    </section>
  );
}