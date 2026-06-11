import React from 'react';

// Panel props interface
export interface PanelProps {
  children: React.ReactNode;
  title?: string;
  className?: string;
  style?: React.CSSProperties;
  headerActions?: React.ReactNode;
  footer?: React.ReactNode;
  collapsible?: boolean;
  defaultExpanded?: boolean;
  loading?: boolean;
  noPadding?: boolean;
}

// Panel state interface for controlled usage
export interface PanelState {
  expanded: boolean;
}

// Status types for the panel
export type PanelStatus = 'default' | 'success' | 'warning' | 'error' | 'info';

// Panel with status indicator
export interface StatusPanelProps extends PanelProps {
  status?: PanelStatus;
  statusMessage?: string;
}

// Grid column layout
export type PanelGridType = 1 | 2 | 3 | 4 | 6 | 12;

// Panel grid props
export interface PanelGridProps {
  children: React.ReactNode;
  columns?: PanelGridType;
  gap?: 'small' | 'medium' | 'large';
  className?: string;
}

// Basic Panel Component
export const Panel: React.FC<PanelProps> = ({
  children,
  title,
  className = '',
  style,
  headerActions,
  footer,
  collapsible = false,
  defaultExpanded = true,
  loading = false,
  noPadding = false,
}) => {
  const [expanded, setExpanded] = React.useState(defaultExpanded);

  const handleToggle = () => {
    if (collapsible) {
      setExpanded((prev) => !prev);
    }
  };

  return (
    <div
      className={`panel ${className} ${!expanded ? 'collapsed' : ''}`}
      style={style}
    >
      {/* Panel Header */}
      {(title || headerActions) && (
        <div className="panel-header" onClick={handleToggle}>
          {title && <h3 className="panel-title">{title}</h3>}
          {headerActions && <div className="panel-actions">{headerActions}</div>}
          {collapsible && (
            <span className={`panel-toggle ${expanded ? 'expanded' : ''}`}>▼</span>
          )}
        </div>
      )}

      {/* Panel Body */}
      <div className={`panel-body ${noPadding ? 'no-padding' : ''}`}>
        {loading && <div className="panel-loading">Loading...</div>}
        {(!loading || children) && (
          <div className={`panel-content ${expanded ? 'visible' : 'hidden'}`}>
            {children}
          </div>
        )}
      </div>

      {/* Panel Footer */}
      {footer && <div className="panel-footer">{footer}</div>}
    </div>
  );
};

// Status Panel Component
export const StatusPanel: React.FC<StatusPanelProps> = ({
  status,
  statusMessage,
  children,
  title,
  className = '',
  ...props
}) => {
  const getBorderColor = () => {
    switch (status) {
      case 'success':
        return '#4caf50';
      case 'warning':
        return '#ff9800';
      case 'error':
        return '#f44336';
      case 'info':
        return '#2196f3';
      default:
        return '#e0e0e0';
    }
  };

  const getBackground = () => {
    switch (status) {
      case 'success':
        return '#f1f8e9';
      case 'warning':
        return '#fff3e0';
      case 'error':
        return '#ffebee';
      case 'info':
        return '#e3f2fd';
      default:
        return '#ffffff';
    }
  };

  const statusColor = getBorderColor();
  const statusBackground = getBackground();

  return (
    <div
      className={`panel status-panel ${status || ''} ${className}`}
      style={{ borderLeft: `4px solid ${statusColor}`, backgroundColor: statusBackground }}
      {...props}
    >
      {(title || statusMessage) && (
        <div className="panel-header">
          {title && <h3 className="panel-title">{title}</h3>}
          {statusMessage && <span className="panel-status-message">{statusMessage}</span>}
        </div>
      )}

      <div className="panel-body">
        {children}
      </div>
    </div>
  );
};

// Panel Grid Component
export const PanelGrid: React.FC<PanelGridProps> = ({
  children,
  columns = 12,
  gap = 'medium',
  className = '',
}) => {
  // Calculate column width based on grid system (12 columns total)
  const getColumnWidth = () => {
    switch (gap) {
      case 'small':
        return { gap: '8px', padding: '4px' };
      case 'large':
        return { gap: '24px', padding: '12px' };
      case 'medium':
      default:
        return { gap: '16px', padding: '8px' };
    }
  };

  const gridStyles = {
    display: 'grid',
    gridTemplateColumns: `repeat(${columns}, 1fr)`,
    ...getColumnWidth(),
  };

  return (
    <div className={`panel-grid ${className}`} style={gridStyles}>
      {React.Children.map(children, (child) => (
        <div
          className="panel-grid-item"
          style={{ padding: getColumnWidth().padding }}
        >
          {child}
        </div>
      ))}
    </div>
  );
};

// Stat Panel Component
export interface StatPanelProps extends PanelProps {
  label?: string;
  value?: string | number;
  subtitle?: string;
  trend?: 'up' | 'down' | 'neutral';
  trendValue?: string | number;
  color?: string;
}

export const StatPanel: React.FC<StatPanelProps> = ({
  label,
  value,
  subtitle,
  trend,
  trendValue,
  children,
  className = '',
}) => {
  const getTrendColor = () => {
    switch (trend) {
      case 'up':
        return '#4caf50';
      case 'down':
        return '#f44336';
      default:
        return '#9e9e9e';
    }
  };

  return (
    <Panel className={`stat-panel ${className}`}>
      {label && <div className="panel-label">{label}</div>}
      <div className="panel-stat-value">{value}</div>
      {(trend || subtitle) && (
        <div className="panel-stat-subtitle">
          {trend && (
            <span
              className={`panel-trend panel-trend-${trend}`}
              style={{ color: getTrendColor() }}
            >
              {trend === 'up' ? '▲' : trend === 'down' ? '▼' : '➖'} {trendValue}
            </span>
          )}
          {subtitle && <span className="panel-subtitle-text">{subtitle}</span>}
        </div>
      )}
      {children}
    </Panel>
  );
};

// Action Panel Component
export interface ActionPanelProps extends PanelProps {
  primaryAction?: React.ReactNode;
  secondaryActions?: React.ReactNode[];
}

export const ActionPanel: React.FC<ActionPanelProps> = ({
  children,
  primaryAction,
  secondaryActions,
  className = '',
}) => {
  return (
    <Panel className={`action-panel ${className}`}>
      {children}
      {(primaryAction || (secondaryActions && secondaryActions.length > 0)) && (
        <div className="panel-actions-bar">
          {primaryAction && <div className="panel-primary-action">{primaryAction}</div>}
          {secondaryActions && secondaryActions.length > 0 && (
            <div className="panel-secondary-actions">
              {secondaryActions.map((action, index) => (
                <div key={index} className="panel-secondary-action">
                  {action}
                </div>
              ))}
            </div>
          )}
        </div>
      )}
    </Panel>
  );
};

// Card Panel Component (for dashboards)
export const CardPanel: React.FC<PanelProps> = ({
  children,
  title,
  className = '',
}) => {
  return (
    <div className={`panel card-panel ${className}`}>
      {title && <h3 className="card-title">{title}</h3>}
      <div className="card-content">{children}</div>
    </div>
  );
};

// Collapsible Panel Hook
export const usePanel = (defaultExpanded: boolean = true) => {
  const [expanded, setExpanded] = React.useState(defaultExpanded);
  const toggle = () => setExpanded((prev) => !prev);
  return { expanded, toggle };
};

export default Panel;
