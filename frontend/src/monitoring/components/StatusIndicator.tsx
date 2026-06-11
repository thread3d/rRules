import React from 'react';

// Status types
export type StatusType = 'online' | 'offline' | 'warning' | 'error' | 'pending' | 'unknown';
export type SeverityLevel = 0 | 1 | 2 | 3 | 4; // 0=success, 4=critical

// Status indicator props
export interface StatusIndicatorProps {
  status: StatusType;
  size?: 'small' | 'medium' | 'large';
  label?: string;
  className?: string;
  showDot?: boolean;
  showLabel?: boolean;
}

// Health status props
export interface HealthStatusProps extends StatusIndicatorProps {
  healthScore?: number; // 0-100
  thresholdWarning?: number;
  thresholdError?: number;
}

// Connection status props
export interface ConnectionStatusProps extends StatusIndicatorProps {
  latency?: number; // in ms
  lastUpdated?: Date;
}

// Aggregated status props (multiple statuses)
export interface AggregatedStatusProps {
  statuses: { id: string; label: string; status: StatusType }[];
  maxItems?: number;
  showCounts?: boolean;
}

// Status indicator component
export const StatusIndicator: React.FC<StatusIndicatorProps> = ({
  status,
  size = 'medium',
  label,
  className = '',
  showDot = true,
  showLabel = true,
}) => {
  // Determine styles based on status
  const getStatusStyles = () => {
    switch (status) {
      case 'online':
        return {
          color: '#4caf50',
          backgroundColor: '#e8f5e9',
          borderColor: '#4caf50',
          pulse: true,
        };
      case 'offline':
        return {
          color: '#f44336',
          backgroundColor: '#ffebee',
          borderColor: '#f44336',
          pulse: false,
        };
      case 'warning':
        return {
          color: '#ff9800',
          backgroundColor: '#fff3e0',
          borderColor: '#ff9800',
          pulse: true,
        };
      case 'error':
        return {
          color: '#f44336',
          backgroundColor: '#ffebee',
          borderColor: '#f44336',
          pulse: false,
        };
      case 'pending':
        return {
          color: '#2196f3',
          backgroundColor: '#e3f2fd',
          borderColor: '#2196f3',
          pulse: true,
        };
      default:
        return {
          color: '#9e9e9e',
          backgroundColor: '#f5f5f5',
          borderColor: '#9e9e9e',
          pulse: false,
        };
    }
  };

  const styles = getStatusStyles();

  // Size classes
  const sizeClasses = {
    small: 'status-indicator-small',
    medium: 'status-indicator-medium',
    large: 'status-indicator-large',
  };

  const dotSize = size === 'small' ? 6 : size === 'large' ? 12 : 8;

  return (
    <div
      className={`status-indicator ${status} ${sizeClasses[size]} ${className}`}
      style={{ ...styles }}
    >
      {showDot && (
        <span
          className="status-dot"
          style={{
            width: dotSize,
            height: dotSize,
            backgroundColor: styles.color,
            animation: styles.pulse ? 'pulse 2s infinite' : 'none',
          }}
        />
      )}
      {showLabel && label && <span className="status-label">{label}</span>}
    </div>
  );
};

// Health status component with score
export const HealthStatus: React.FC<HealthStatusProps> = ({
  status,
  healthScore = 100,
  thresholdWarning = 75,
  thresholdError = 50,
  size = 'medium',
  label = 'Health',
}) => {
  // Determine effective status based on score
  const effectiveStatus =
    healthScore < thresholdError ? 'error' : healthScore < thresholdWarning ? 'warning' : status;

  // Calculate bar percentage
  const barWidth = `${healthScore}%`;

  return (
    <div className="health-status">
      <div className="health-header">
        <StatusIndicator
          status={effectiveStatus}
          size={size}
          label={label}
        />
        <span className="health-score">{healthScore}%</span>
      </div>
      <div className="health-bar-container">
        <div
          className="health-bar"
          style={{
            width: barWidth,
            backgroundColor:
              effectiveStatus === 'error' ? '#f44336' : effectiveStatus === 'warning' ? '#ff9800' : '#4caf50',
          }}
        />
      </div>
    </div>
  );
};

// Connection status component
export const ConnectionStatus: React.FC<ConnectionStatusProps> = ({
  status,
  latency,
  lastUpdated,
  size = 'medium',
  label = 'Connection',
}) => {
  const formatLatency = (ms?: number) => {
    if (!ms || ms < 0) return '-';
    if (ms < 1000) return `${ms}ms`;
    return `${(ms / 1000).toFixed(1)}s`;
  };

  const formatLastUpdated = (date?: Date) => {
    if (!date) return 'Never';
    const now = new Date();
    const diff = now.getTime() - date.getTime();
    const minutes = Math.floor(diff / 60000);

    if (minutes < 1) return 'Just now';
    if (minutes < 60) return `${minutes}m ago`;
    const hours = Math.floor(minutes / 60);
    if (hours < 24) return `${hours}h ${minutes % 60}m ago`;
    return date.toLocaleDateString();
  };

  return (
    <div className="connection-status">
      <StatusIndicator status={status} size={size} label={label} />
      {latency !== undefined && (
        <span className="connection-latency" title={`Latency: ${formatLatency(latency)}`}>
          {formatLatency(latency)}
        </span>
      )}
      {lastUpdated && (
        <span
          className="connection-updated"
          title={`Last updated: ${lastUpdated.toLocaleString()}`}
        >
          {formatLastUpdated(lastUpdated)}
        </span>
      )}
    </div>
  );
};

// Aggregated status component
export const AggregatedStatus: React.FC<AggregatedStatusProps> = ({
  statuses,
  maxItems = 5,
  showCounts = true,
}) => {
  const displayItems = statuses.slice(0, maxItems);
  const remainingCount = Math.max(0, statuses.length - maxItems);

  // Count by status
  const counts = statuses.reduce(
    (acc, s) => {
      acc[s.status] = (acc[s.status] || 0) + 1;
      return acc;
    },
    {} as Record<StatusType, number>
  );

  // Determine overall status (highest priority wins)
  const getOverallStatus = (): StatusType => {
    for (const s of statuses) {
      if (s.status === 'error') return 'error';
      if (s.status === 'warning') return 'warning';
      if (s.status === 'pending') return 'pending';
      if (s.status === 'offline') return 'offline';
    }
    return 'online';
  };

  const overallStatus = getOverallStatus();

  return (
    <div className="aggregated-status">
      <div className="aggregated-header">
        <StatusIndicator status={overallStatus} label={`Aggregated Status`} />
        {showCounts && (
          <div className="status-counts">
            {counts.online! > 0 && (
              <span className="count online" title={`${counts.online} online`}>
                ● {counts.online}
              </span>
            )}
            {counts.warning! > 0 && (
              <span className="count warning" title={`${counts.warning} warnings`}>
                ● {counts.warning}
              </span>
            )}
            {counts.error! > 0 && (
              <span className="count error" title={`${counts.error} errors`}>
                ● {counts.error}
              </span>
            )}
            {(counts.offline || 0) > 0 && (
              <span className="count offline" title={`${counts.offline || 0} offline`}>
                ● {counts.offline}
              </span>
            )}
          </div>
        )}
      </div>

      <div className="aggregated-list">
        {displayItems.map((item) => (
          <div key={item.id} className="status-item">
            <StatusIndicator status={item.status} size="small" label={item.label} />
          </div>
        ))}
        {remainingCount > 0 && (
          <span className="status-remaining">+{remainingCount} more</span>
        )}
      </div>
    </div>
  );
};

// Status badge component
export const StatusBadge: React.FC<StatusIndicatorProps> = ({
  status,
  size = 'small',
  label,
}) => {
  return (
    <span className={`status-badge ${status}`}>
      <span
        className="badge-dot"
        style={{
          width: size === 'small' ? 8 : 12,
          height: size === 'small' ? 8 : 12,
          backgroundColor:
            status === 'online'
              ? '#4caf50'
              : status === 'warning'
              ? '#ff9800'
              : status === 'error'
              ? '#f44336'
              : status === 'pending'
              ? '#2196f3'
              : '#9e9e9e',
        }}
      />
      {label}
    </span>
  );
};

// Loading indicator
export const LoadingIndicator: React.FC<{ size?: 'small' | 'medium' | 'large'; label?: string }> = ({
  size = 'medium',
  label = 'Loading...',
}) => {
  return (
    <div className="loading-indicator">
      <div
        className="spinner"
        style={{
          width: size === 'small' ? 16 : size === 'large' ? 40 : 24,
          height: size === 'small' ? 16 : size === 'large' ? 40 : 24,
        }}
      />
      <span className="loading-text">{label}</span>
    </div>
  );
};

// Error indicator
export const ErrorIndicator: React.FC<{ message?: string; onRetry?: () => void }> = ({
  message,
  onRetry,
}) => {
  return (
    <div className="error-indicator">
      <StatusIndicator status="error" label={message || 'Error'} />
      {onRetry && <button onClick={onRetry}>Retry</button>}
    </div>
  );
};

// Status dot (just the dot, no label)
export const StatusDot: React.FC<{ status: StatusType; size?: number }> = ({
  status,
  size = 8,
}) => {
  return (
    <span
      className={`status-dot ${status}`}
      style={{
        width: size,
        height: size,
        backgroundColor:
          status === 'online'
            ? '#4caf50'
            : status === 'warning'
            ? '#ff9800'
            : status === 'error'
            ? '#f44336'
            : status === 'pending'
            ? '#2196f3'
            : '#9e9e9e',
        animation: status === 'online' || status === 'warning' || status === 'pending' ? 'pulse 2s infinite' : 'none',
      }}
    />
  );
};

export default StatusIndicator;
