import React from 'react';
import {
  ResponsiveContainer,
  LineChart,
  BarChart,
  AreaChart,
  PieChart,
  Line,
  Bar,
  Area,
  Pie,
  Cell,
  XAxis,
  YAxis,
  CartesianGrid,
  Tooltip,
  Legend,
  ReferenceLine,
} from 'recharts';
import type {
  ResponsiveContainerProps as ReResponsiveContainerProps,
  LineChartProps as ReLineChartProps,
  BarChartProps as ReBarChartProps,
  AreaChartProps as ReAreaChartProps,
  PieChartProps as RePieChartProps,
  LineProps as ReLineProps,
  BarProps as ReBarProps,
  AreaProps as ReAreaProps,
  PieProps as RePieProps,
  CellProps as ReCellProps,
  XAxisProps as ReXAxisProps,
  YAxisProps as ReYAxisProps,
  CartesianGridProps as ReCartesianGridProps,
  TooltipProps as ReTooltipProps,
  LegendProps as ReLegendProps,
  ReferenceLineProps as ReReferenceLineProps,
} from 'recharts';

// Data types
export interface ChartData {
  [key: string]: string | number | Date;
}

export interface ChartSeries {
  dataKey: string;
  name?: string;
  color?: string;
  type?: 'line' | 'bar' | 'area';
  strokeWidth?: number;
  fillOpacity?: number;
}

// Base chart props
export interface ChartContainerProps extends ReResponsiveContainerProps {
  children: React.ReactNode;
  width?: string | number;
  height?: string | number;
  className?: string;
}

export interface CartesianChartProps {
  data: ChartData[];
  series: ChartSeries[];
  xKey: string;
  title?: string;
  showGrid?: boolean;
  showTooltip?: boolean;
  showLegend?: boolean;
  xLabel?: string;
  yLabel?: string;
  className?: string;
}

export interface LineChartProps extends CartesianChartProps {
  strokeWidth?: number;
  connectNulls?: boolean;
}

export interface BarChartProps extends CartesianChartProps {
  barSize?: number;
  maxBarSize?: number;
  stackOffset?: 'none' | 'expand' | 'sign' | 'wiggle';
}

export interface AreaChartProps extends CartesianChartProps {
  fillOpacity?: number;
  strokeWidth?: number;
  connectNulls?: boolean;
}

export interface PieChartProps {
  data: ChartData[];
  dataKey: string;
  nameKey: string;
  cx?: string | number;
  cy?: string | number;
  innerRadius?: string | number;
  outerRadius?: string | number;
  title?: string;
  showTooltip?: boolean;
  showLegend?: boolean;
  className?: string;
}

export interface ChartProps {
  type?: 'line' | 'bar' | 'area' | 'pie';
  data: ChartData[];
  series?: ChartSeries[];
  xKey?: string;
  yKeys?: string[];
  title?: string;
  width?: string | number;
  height?: string | number;
  className?: string;
  [key: string]: any;
}

// Chart Container Component
export const ChartContainer: React.FC<ChartContainerProps> = ({
  children,
  width = '100%',
  height = 300,
  className = '',
  ...props
}) => {
  return (
    <ResponsiveContainer width={width} height={height} className={`chart-container ${className}`} {...props}>
      {children}
    </ResponsiveContainer>
  );
};

// Cartesian Chart Base Component
export const CartesianChart: React.FC<CartesianChartProps> = ({
  data,
  series,
  xKey,
  title,
  showGrid = true,
  showTooltip = true,
  showLegend = true,
  xLabel,
  yLabel,
  className = '',
}) => {
  return (
    <ChartContainer width="100%" height={350} className={className}>
      {series && series.length > 0 ? (
        <LineChart data={data} margin={{ top: 20, right: 30, left: 20, bottom: 50 }}>
          {showGrid && <CartesianGrid strokeDasharray="3 3" />}
          <XAxis dataKey={xKey} label={xLabel} />
          <YAxis label={yLabel} />
          {showTooltip && <Tooltip />}
          {showLegend && <Legend />}
          {series.map((s, index) => (
            <Line
              key={s.dataKey}
              type="monotone"
              dataKey={s.dataKey}
              name={s.name || s.dataKey}
              stroke={s.color || `hsl(${index * 60}, 70%, 50%)`}
              strokeWidth={s.strokeWidth || 2}
            />
          ))}
        </LineChart>
      ) : null}
    </ChartContainer>
  );
};

// Line Chart Component
export const LineChartComponent: React.FC<LineChartProps> = ({
  data,
  series,
  xKey,
  title,
  showGrid = true,
  showTooltip = true,
  showLegend = true,
  xLabel,
  yLabel,
  strokeWidth = 2,
  connectNulls = false,
  className = '',
}) => {
  return (
    <ChartContainer width="100%" height={350} className={className}>
      <LineChart data={data} margin={{ top: 20, right: 30, left: 20, bottom: 50 }}>
        {showGrid && <CartesianGrid strokeDasharray="3 3" />}
        <XAxis dataKey={xKey} label={xLabel} />
        <YAxis label={yLabel} />
        {showTooltip && <Tooltip />}
        {showLegend && <Legend />}
        {series?.map((s, index) => (
          <Line
            key={s.dataKey}
            type="monotone"
            dataKey={s.dataKey}
            name={s.name || s.dataKey}
            stroke={s.color || `hsl(${index * 60}, 70%, 50%)`}
            strokeWidth={strokeWidth}
            connectNulls={connectNulls}
          />
        ))}
      </LineChart>
    </ChartContainer>
  );
};

// Bar Chart Component
export const BarChartComponent: React.FC<BarChartProps> = ({
  data,
  series,
  xKey,
  title,
  showGrid = true,
  showTooltip = true,
  showLegend = true,
  xLabel,
  yLabel,
  barSize = 40,
  maxBarSize = 50,
  stackOffset = 'none',
  className = '',
}) => {
  return (
    <ChartContainer width="100%" height={350} className={className}>
      <BarChart data={data} margin={{ top: 20, right: 30, left: 20, bottom: 50 }}>
        {showGrid && <CartesianGrid strokeDasharray="3 3" />}
        <XAxis dataKey={xKey} label={xLabel} />
        <YAxis label={yLabel} />
        {showTooltip && <Tooltip />}
        {showLegend && <Legend />}
        {series?.map((s, index) => (
          <Bar
            key={s.dataKey}
            dataKey={s.dataKey}
            name={s.name || s.dataKey}
            fill={s.color || `hsl(${index * 60}, 70%, 50%)`}
            barSize={barSize}
            maxBarSize={maxBarSize}
          />
        ))}
      </BarChart>
    </ChartContainer>
  );
};

// Area Chart Component
export const AreaChartComponent: React.FC<AreaChartProps> = ({
  data,
  series,
  xKey,
  title,
  showGrid = true,
  showTooltip = true,
  showLegend = true,
  xLabel,
  yLabel,
  fillOpacity = 0.5,
  strokeWidth = 2,
  connectNulls = false,
  className = '',
}) => {
  return (
    <ChartContainer width="100%" height={350} className={className}>
      <AreaChart data={data} margin={{ top: 20, right: 30, left: 20, bottom: 50 }}>
        {showGrid && <CartesianGrid strokeDasharray="3 3" />}
        <XAxis dataKey={xKey} label={xLabel} />
        <YAxis label={yLabel} />
        {showTooltip && <Tooltip />}
        {showLegend && <Legend />}
        {series?.map((s, index) => (
          <Area
            key={s.dataKey}
            type="monotone"
            dataKey={s.dataKey}
            name={s.name || s.dataKey}
            fill={s.color || `hsl(${index * 60}, 70%, 50%)`}
            fillOpacity={fillOpacity}
            stroke={s.color || `hsl(${index * 60}, 70%, 50%)`}
            strokeWidth={strokeWidth}
            connectNulls={connectNulls}
          />
        ))}
      </AreaChart>
    </ChartContainer>
  );
};

// Pie Chart Component
export const PieChartComponent: React.FC<PieChartProps> = ({
  data,
  dataKey,
  nameKey,
  cx = '50%',
  cy = '50%',
  innerRadius = 0,
  outerRadius = 100,
  title,
  showTooltip = true,
  showLegend = true,
  className = '',
}) => {
  const COLORS = ['#0088FE', '#00C49F', '#FFBB28', '#FF8042', '#8884D8', '#82CA9D'];

  return (
    <ChartContainer width="100%" height={350} className={className}>
      <PieChart margin={{ top: 20, right: 30, left: 20, bottom: 50 }}>
        {showTooltip && <Tooltip />}
        {showLegend && <Legend />}
        <Pie
          data={data}
          cx={cx}
          cy={cy}
          innerRadius={innerRadius}
          outerRadius={outerRadius}
          dataKey={dataKey}
          nameKey={nameKey}
        >
          {data?.map((entry, index) => (
            <Cell key={`cell-${index}`} fill={COLORS[index % COLORS.length]} />
          ))}
        </Pie>
      </PieChart>
    </ChartContainer>
  );
};

// Generic Chart Component (wrapper)
export const Chart: React.FC<ChartProps> = ({
  type = 'line',
  data,
  series,
  xKey = 'name',
  yKeys = ['value'],
  title,
  width = '100%',
  height = 350,
  className = '',
  ...props
}) => {
  if (type === 'pie') {
    return <PieChartComponent data={data} dataKey={yKeys[0]} nameKey={xKey} {...props} />;
  }

  const chartSeries: ChartSeries[] =
    series ||
    yKeys.map((key, index) => ({
      dataKey: key,
      color: `hsl(${index * 60}, 70%, 50%)`,
      type: 'line',
    }));

  switch (type) {
    case 'bar':
      return <BarChartComponent data={data} series={chartSeries} xKey={xKey} {...props} />;
    case 'area':
      return <AreaChartComponent data={data} series={chartSeries} xKey={xKey} {...props} />;
    case 'line':
    default:
      return <LineChartComponent data={data} series={chartSeries} xKey={xKey} {...props} />;
  }
};

// Simple SVG fallback Chart (when recharts not available)
export const SimpleSvgChart: React.FC<{
  data: { label: string; value: number }[];
  width?: number;
  height?: number;
}> = ({ data, width = 400, height = 300 }) => {
  if (data.length === 0) return null;

  const maxValue = Math.max(...data.map((d) => d.value));
  const barWidth = width / data.length - 20;

  return (
    <svg width={width} height={height} className="simple-svg-chart">
      {data.map((d, i) => {
        const barHeight = (d.value / maxValue) * (height - 50);
        const x = i * (barWidth + 20) + 10;
        const y = height - barHeight - 30;

        return (
          <g key={i}>
            <rect x={x} y={y} width={barWidth} height={barHeight} fill="#8884d8" rx="4" />
            <text
              x={x + barWidth / 2}
              y={height - 10}
              textAnchor="middle"
              fontSize="12"
              fill="#666"
            >
              {d.label}
            </text>
            <text
              x={x + barWidth / 2}
              y={y - 5}
              textAnchor="middle"
              fontSize="10"
              fill="#333"
            >
              {d.value}
            </text>
          </g>
        );
      })}
    </svg>
  );
};

export default Chart;
