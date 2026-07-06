// Phase 28 (Plan 28-02): Vitest global setup.
// Registers @testing-library/jest-dom matchers (toBeInTheDocument, toHaveTextContent, etc.)
// globally so component tests can use them without per-file imports.
import '@testing-library/jest-dom'