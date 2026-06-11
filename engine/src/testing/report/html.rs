//! HTML report generator for test results with embedded CSS
//!
//! This module generates self-contained HTML reports for test execution results,
//! including summary statistics and detailed test outcome information.

use crate::testing::RuleCoverage;
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// Result of a single test case
#[derive(Debug, Clone)]
pub struct TestResult {
    /// Name of the test
    pub name: String,
    /// Whether the test passed
    pub passed: bool,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Optional error message if test failed
    pub error: Option<String>,
    /// Additional details about the test
    pub details: HashMap<String, String>,
}

/// Summary statistics for a test run
#[derive(Debug, Clone)]
pub struct ReportSummary {
    /// Total number of tests
    pub total: usize,
    /// Number of passed tests
    pub passed: usize,
    /// Number of failed tests
    pub failed: usize,
    /// Duration in milliseconds
    pub duration_ms: u64,
    /// Test run timestamp
    pub timestamp: DateTime<Utc>,
}

/// HTML report generator for test results
#[derive(Debug, Clone)]
pub struct HtmlReport {
    /// Results of all test cases
    tests: Vec<TestResult>,
    /// Summary statistics
    summary: ReportSummary,
    /// Optional rule coverage data
    coverage: Option<RuleCoverage>,
}

impl HtmlReport {
    /// Create a new HTML report generator
    ///
    /// # Arguments
    /// * `tests` - Vector of test results
    /// * `summary` - Summary statistics for the test run
    pub fn new(tests: Vec<TestResult>, summary: ReportSummary) -> Self {
        HtmlReport {
            tests,
            summary,
            coverage: None,
        }
    }

    /// Set rule coverage data for this report
    pub fn with_coverage(mut self, coverage: RuleCoverage) -> Self {
        self.coverage = Some(coverage);
        self
    }

    /// Generate HTML content for the report
    ///
    /// The HTML includes embedded CSS for styling and is fully self-contained.
    ///
    /// # Returns
    /// A complete HTML document as a string
    pub fn to_html(&self) -> String {
        let timestamp = self.summary.timestamp.format("%Y-%m-%d %H:%M:%S UTC").to_string();
        let total = self.summary.total;
        let passed = self.summary.passed;
        let failed = self.summary.failed;
        let duration_ms = self.summary.duration_ms;

        let coverage_section = if let Some(ref coverage) = self.coverage {
            let eval_coverage = coverage.evaluation_coverage();
            let fire_coverage = coverage.firing_coverage();

            format!(
                r#"<div class="coverage-card">
    <h2>Rule Coverage</h2>
    <div class="coverage-grid">
        <div>
            <strong>Evaluation Coverage:</strong><br>
            <span class="stat-value">{eval_coverage:.1}%</span>
            <div class="metric-bar"><div class="metric-fill metric-covered" style="width: {eval_coverage}%"></div></div>
        </div>
        <div>
            <strong>Firing Coverage:</strong><br>
            <span class="stat-value">{fire_coverage:.1}%</span>
            <div class="metric-bar"><div class="metric-fill metric-uncovered" style="width: {fire_coverage}%"></div></div>
        </div>
    </div>
</div>"#
            )
        } else {
            String::new()
        };

        let test_table = self.render_test_table();

        format!(
            r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>rRules Test Report</title>
    <style>
        * {{
            box-sizing: border-box;
            margin: 0;
            padding: 0;
        }}

        body {{
            font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, sans-serif;
            line-height: 1.6;
            color: #333;
            background-color: #f5f7fa;
            padding: 20px;
        }}

        .container {{
            max-width: 1200px;
            margin: 0 auto;
        }}

        h1 {{
            color: #2c3e50;
            margin-bottom: 24px;
            border-bottom: 2px solid #3498db;
            padding-bottom: 12px;
        }}

        h2 {{
            color: #34495e;
            margin: 24px 0 16px;
            font-size: 1.25em;
        }}

        .summary-card {{
            background: white;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            padding: 20px;
            margin-bottom: 24px;
        }}

        .summary-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
            gap: 16px;
        }}

        .stat-box {{
            text-align: center;
            padding: 16px;
            background: #f8f9fa;
            border-radius: 6px;
        }}

        .stat-value {{
            font-size: 2em;
            font-weight: bold;
            display: block;
        }}

        .stat-label {{
            color: #7f8c8d;
            font-size: 0.875em;
            margin-top: 4px;
        }}

        .passed .stat-value {{ color: #27ae60; }}
        .failed .stat-value {{ color: #e74c3c; }}
        .total .stat-value {{ color: #3498db; }}
        .duration .stat-value {{ color: #9b59b6; }}

        .timestamp {{
            text-align: center;
            color: #7f8c8d;
            font-size: 0.875em;
            margin-bottom: 20px;
        }}

        .coverage-card {{
            background: white;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            padding: 20px;
            margin-bottom: 24px;
        }}

        .coverage-grid {{
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
            gap: 16px;
            margin-top: 16px;
        }}

        .metric-bar {{
            height: 8px;
            background: #e9ecef;
            border-radius: 4px;
            overflow: hidden;
        }}

        .metric-fill {{
            height: 100%;
            transition: width 0.3s ease;
        }}

        .metric-passed {{ background: #27ae60; }}
        .metric-covered {{ background: #3498db; }}
        .metric-uncovered {{ background: #f39c12; }}

        table {{
            width: 100%;
            border-collapse: collapse;
            background: white;
            border-radius: 8px;
            box-shadow: 0 2px 4px rgba(0,0,0,0.1);
            overflow: hidden;
        }}

        th, td {{
            padding: 12px 16px;
            text-align: left;
            border-bottom: 1px solid #e9ecef;
        }}

        th {{
            background: #34495e;
            color: white;
            font-weight: 600;
            font-size: 0.875em;
            text-transform: uppercase;
            letter-spacing: 0.5px;
        }}

        tr:hover {{
            background: #f8f9fa;
        }}

        .test-passed {{
            color: #27ae60;
            font-weight: 600;
        }}

        .test-failed {{
            color: #e74c3c;
            font-weight: 600;
        }}

        .duration-cell {{
            color: #7f8c8d;
            font-family: 'Menlo', 'Monaco', 'Courier New', monospace;
        }}

        .error-details {{
            margin-top: 8px;
            padding: 12px;
            background: #fff5f5;
            border-left: 4px solid #e74c3c;
            font-family: 'Menlo', 'Monaco', 'Courier New', monospace;
            font-size: 0.85em;
            color: #c0392b;
        }}

        .details-table {{
            margin-top: 16px;
        }}

        .details-table th, .details-table td {{
            padding: 8px 12px;
            border-bottom: none;
        }}

        .details-table tr:last-child td {{
            border-bottom: none;
        }}

        .empty-state {{
            text-align: center;
            padding: 40px;
            color: #7f8c8d;
        }}
    </style>
</head>
<body>
    <div class="container">
        <h1>rRules Test Report</h1>
        <p class="timestamp">Generated on {timestamp}</p>

        <div class="summary-card">
            <div class="summary-grid">
                <div class="stat-box total">
                    <span class="stat-value">{total}</span>
                    <span class="stat-label">Total Tests</span>
                </div>
                <div class="stat-box passed">
                    <span class="stat-value">{passed}</span>
                    <span class="stat-label">Passed</span>
                </div>
                <div class="stat-box failed">
                    <span class="stat-value">{failed}</span>
                    <span class="stat-label">Failed</span>
                </div>
                <div class="stat-box duration">
                    <span class="stat-value">{duration_ms}ms</span>
                    <span class="stat-label">Duration</span>
                </div>
            </div>
        </div>

        {coverage_section}

        <h2>Test Results</h2>
        {test_table}
    </div>
</body>
</html>"#
        )
    }

    /// Generate the test results table HTML
    fn render_test_table(&self) -> String {
        if self.tests.is_empty() {
            return r#"<div class="empty-state">No tests were run.</div>"#.to_string();
        }

        let mut rows = String::new();

        for test in &self.tests {
            let status_class = if test.passed { "test-passed" } else { "test-failed" };
            let status_text = if test.passed { "Passed" } else { "Failed" };

            let error_html = if let Some(ref error) = test.error {
                format!(
                    r#"<div class="error-details"><strong>Error:</strong><br><pre>{}</pre></div>"#,
                    escape_html(error)
                )
            } else {
                String::new()
            };

            let details_html = if !test.details.is_empty() {
                let detail_rows: Vec<String> = test
                    .details
                    .iter()
                    .map(|(k, v)| format!(r#"<tr><th>{}</th><td>{}</td></tr>"#, escape_html(k), escape_html(v)))
                    .collect();

                format!(
                    r#"<table class="details-table">
<tr><th>Detail</th><td>{}</td></tr>
</table>"#,
                    detail_rows.join("\n")
                )
            } else {
                String::new()
            };

            rows.push_str(&format!(
                r#"<tr>
    <td>{name}</td>
    <td class="{status_class}">{status}</td>
    <td class="duration-cell">{duration_ms}ms</td>
    <td>{error_html}{details_html}</td>
</tr>"#,
                name = test.name,
                status = if test.passed { "Passed" } else { "Failed" },
                duration_ms = test.duration_ms
            ));
        }

        format!(
            r#"<table>
<thead>
    <tr>
        <th>Name</th>
        <th>Status</th>
        <th>Duration</th>
        <th>Details</th>
    </tr>
</thead>
<tbody>
{rows}
</tbody>
</table>"#
        )
    }
}

/// Escape HTML special characters
fn escape_html(s: &str) -> String {
    s.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace("\"", "&quot;")
        .replace("'", "&#x27;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_generates_valid_html() {
        let results = vec![TestResult {
            name: "test_example".to_string(),
            passed: true,
            duration_ms: 100,
            error: None,
            details: HashMap::new(),
        }];

        let summary = ReportSummary {
            total: 1,
            passed: 1,
            failed: 0,
            duration_ms: 100,
            timestamp: Utc::now(),
        };

        let report = HtmlReport::new(results, summary);
        let html = report.to_html();

        // Basic HTML structure checks
        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<html"));
        assert!(html.contains("</html>"));
        assert!(html.contains("<head>"));
        assert!(html.contains("</head>"));
        assert!(html.contains("<body>"));
        assert!(html.contains("</body>"));

        // Content checks
        assert!(html.contains("rRules Test Report"));
    }

    #[test]
    fn test_report_includes_summary_stats() {
        let results = vec![TestResult {
            name: "test_a".to_string(),
            passed: true,
            duration_ms: 50,
            error: None,
            details: HashMap::new(),
        }];

        let summary = ReportSummary {
            total: 1,
            passed: 1,
            failed: 0,
            duration_ms: 50,
            timestamp: Utc::now(),
        };

        let report = HtmlReport::new(results, summary);
        let html = report.to_html();

        assert!(html.contains("Total Tests"));
        assert!(html.contains("Passed"));
        assert!(html.contains("Failed"));
        assert!(html.contains("Duration"));

        // Check values are present
        assert!(html.contains("1"));
    }

    #[test]
    fn test_report_includes_test_details() {
        let mut details = HashMap::new();
        details.insert("rule_count".to_string(), "5".to_string());
        details.insert("facts_inserted".to_string(), "3".to_string());

        let results = vec![TestResult {
            name: "complex_test".to_string(),
            passed: false,
            duration_ms: 200,
            error: Some("Assertion failed".to_string()),
            details,
        }];

        let summary = ReportSummary {
            total: 1,
            passed: 0,
            failed: 1,
            duration_ms: 200,
            timestamp: Utc::now(),
        };

        let report = HtmlReport::new(results, summary);
        let html = report.to_html();

        assert!(html.contains("complex_test"));
        assert!(html.contains("Failed"));
        assert!(html.contains("Assertion failed"));
        assert!(html.contains("rule_count"));
        assert!(html.contains("5"));
    }

    #[test]
    fn test_report_with_coverage() {
        let results = vec![TestResult {
            name: "test_example".to_string(),
            passed: true,
            duration_ms: 100,
            error: None,
            details: HashMap::new(),
        }];

        let summary = ReportSummary {
            total: 1,
            passed: 1,
            failed: 0,
            duration_ms: 100,
            timestamp: Utc::now(),
        };

        let mut coverage = RuleCoverage::new(10);
        coverage.record_evaluation("rule1");
        coverage.record_firing("rule1");
        coverage.record_evaluation("rule2");

        let report = HtmlReport::new(results, summary).with_coverage(coverage);
        let html = report.to_html();

        assert!(html.contains("Rule Coverage"));
        assert!(html.contains("Evaluation Coverage"));
        assert!(html.contains("Firing Coverage"));
    }

    #[test]
    fn test_empty_test_list() {
        let summary = ReportSummary {
            total: 0,
            passed: 0,
            failed: 0,
            duration_ms: 0,
            timestamp: Utc::now(),
        };

        let report = HtmlReport::new(Vec::new(), summary);
        let html = report.to_html();

        assert!(html.contains("No tests were run"));
    }

    #[test]
    fn test_escape_html() {
        use crate::testing::report::html::escape_html;

        assert_eq!(escape_html("hello"), "hello");
        assert_eq!(escape_html("<script>"), "&lt;script&gt;");
        assert_eq!(escape_html("a & b"), "a &amp; b");
    }
}
