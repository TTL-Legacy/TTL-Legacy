# Frontend Development Guide

This guide covers everything you need to know about developing the TTL-Legacy frontend dashboard.

## Table of Contents

1. [Prerequisites](#prerequisites)
2. [Environment Setup](#environment-setup)
3. [Running the Development Server](#running-the-development-server)
4. [Project Structure](#project-structure)
5. [Running Tests](#running-tests)
6. [Running E2E Tests](#running-e2e-tests)
7. [Development Workflow](#development-workflow)
8. [Common Tasks](#common-tasks)
9. [Troubleshooting](#troubleshooting)

## Prerequisites

Before you begin, ensure you have the following installed:

| Tool | Version | Installation |
|---|---|---|
| Node.js | 18+ | [nodejs.org](https://nodejs.org) |
| npm | 9+ | Bundled with Node.js |
| Git | latest | [git-scm.com](https://git-scm.com) |
| Backend API | running | See [CONTRIBUTING.md](../CONTRIBUTING.md#local-dev-setup-step-by-step) |

### Verify Installation

```bash
node --version  # Should be v18.0.0 or higher
npm --version   # Should be v9.0.0 or higher
git --version   # Any recent version
```

## Environment Setup

### 1. Install Dependencies

Navigate to the frontend directory and install all dependencies:

```bash
cd frontend
npm install
```

This installs all required packages listed in `package.json`:
- **React** 18.3.1 - UI library
- **Vite** 5.3.4 - Development server and build tool
- **TypeScript** 5.5.3 - Type safety
- **Jest** 29.7.0 - Unit testing framework
- **Testing Library** - Component testing utilities

### 2. Environment Variables

The frontend requires certain environment variables to connect to the backend and Stellar network.

**Create a `.env.local` file in the `frontend/` directory:**

```bash
cat > frontend/.env.local << 'EOF'
# Backend API Configuration
VITE_API_URL=http://localhost:3000
VITE_API_TIMEOUT=30000

# Stellar Network Configuration
VITE_STELLAR_RPC_URL=http://localhost:8000
VITE_STELLAR_NETWORK_PASSPHRASE=Test SDF Network ; September 2015
VITE_STELLAR_HORIZON_URL=http://localhost:8000

# Feature Flags
VITE_ENABLE_PASSKEY=true
VITE_ENABLE_RECOVERY_CODES=true
VITE_ENABLE_2FA=true

# Application Settings
VITE_APP_NAME=TTL-Legacy
VITE_APP_VERSION=1.0.0
VITE_DEBUG_MODE=false
EOF
```

**Environment Variables Reference:**

| Variable | Description | Default |
|----------|---|---|
| `VITE_API_URL` | Backend API base URL | `http://localhost:3000` |
| `VITE_API_TIMEOUT` | API request timeout in ms | `30000` |
| `VITE_STELLAR_RPC_URL` | Stellar RPC endpoint | `http://localhost:8000` |
| `VITE_STELLAR_NETWORK_PASSPHRASE` | Stellar network identifier | `Test SDF Network ; September 2015` |
| `VITE_STELLAR_HORIZON_URL` | Stellar Horizon API URL | `http://localhost:8000` |
| `VITE_ENABLE_PASSKEY` | Enable passkey authentication | `true` |
| `VITE_ENABLE_RECOVERY_CODES` | Enable recovery codes | `true` |
| `VITE_ENABLE_2FA` | Enable two-factor authentication | `true` |
| `VITE_APP_NAME` | Application name | `TTL-Legacy` |
| `VITE_APP_VERSION` | Application version | `1.0.0` |
| `VITE_DEBUG_MODE` | Enable debug logging | `false` |

### 3. Verify Backend Connectivity

Before starting the development server, ensure the backend is running:

```bash
# Check backend health
curl http://localhost:3000/health

# Should return 200 OK with health status
```

If this fails, ensure the backend is running (see [CONTRIBUTING.md](../CONTRIBUTING.md#local-dev-setup-step-by-step)).

## Running the Development Server

### Start the Development Server

```bash
cd frontend
npm run dev
```

The development server will start at `http://localhost:5173` with the following features:
- **Hot Module Replacement (HMR)** - Changes appear instantly without page reload
- **TypeScript Checking** - Real-time type checking
- **Fast Refresh** - Preserves component state during edits
- **Source Maps** - Accurate line numbers in browser DevTools

**Output:**
```
  VITE v5.3.4  ready in 156 ms

  ➜  Local:   http://localhost:5173/
  ➜  press h to show help
```

### Accessing the Application

1. Open your browser to `http://localhost:5173`
2. Log in or create an account using passkey authentication
3. Start developing!

### Stopping the Development Server

Press `Ctrl+C` in the terminal to stop the server.

## Project Structure

```
frontend/
├── src/
│   ├── components/       # Reusable React components
│   ├── __tests__/        # Unit tests for components
│   ├── __mocks__/        # Mock implementations for testing
│   └── setupTests.ts     # Jest configuration and globals
├── e2e/                  # End-to-end tests
├── package.json          # Dependencies and scripts
├── tsconfig.json         # TypeScript configuration
└── vite.config.ts        # Vite build configuration (if exists)
```

### Key Directories

**`src/components/`** - React components for the UI
- Each component should have its own folder
- Include component-specific styles
- Example: `src/components/VaultCard/VaultCard.tsx`

**`src/__tests__/`** - Unit tests using Jest + Testing Library
- Test files should mirror component structure
- Naming: `ComponentName.test.tsx`
- Example: `src/__tests__/VaultCard.test.tsx`

**`src/__mocks__/`** - Mock implementations
- Mock API responses
- Mock external dependencies
- Example: `src/__mocks__/stellarMock.ts`

**`e2e/`** - End-to-end tests (Playwright, Cypress, or similar)
- Full user workflows
- Integration tests
- Example: `e2e/vault-creation.spec.ts`

## Running Tests

### Unit Tests with Jest

Run all unit tests:

```bash
npm test
```

Run tests in watch mode (re-runs on file changes):

```bash
npm run test:watch
```

Run tests for a specific file:

```bash
npm test -- VaultCard.test.tsx
```

Run tests with coverage report:

```bash
npm test -- --coverage
```

### Test Configuration

Jest is configured in `package.json`:
```json
{
  "jest": {
    "testEnvironment": "jsdom",
    "preset": "ts-jest",
    "setupFilesAfterFramework": ["<rootDir>/src/setupTests.ts"]
  }
}
```

### Writing Tests

Example test using React Testing Library:

```typescript
import { render, screen } from '@testing-library/react';
import { VaultCard } from './VaultCard';

describe('VaultCard', () => {
  it('displays vault information', () => {
    const vault = {
      id: 'v1',
      owner: 'test-owner',
      balance: 1000,
    };

    render(<VaultCard vault={vault} />);

    expect(screen.getByText('test-owner')).toBeInTheDocument();
    expect(screen.getByText('1000')).toBeInTheDocument();
  });
});
```

### Test Best Practices

1. **Test behavior, not implementation** - Focus on what users see and do
2. **Use semantic queries** - Prefer `getByRole`, `getByLabelText` over `getByTestId`
3. **Avoid testing implementation details** - Don't test internal state directly
4. **Keep tests focused** - One concept per test
5. **Use descriptive names** - Test names should describe the expected behavior

## Running E2E Tests

End-to-end tests verify the entire application flow from user interaction to backend response.

### Prerequisites for E2E Tests

- Development server running: `npm run dev`
- Backend API running on `http://localhost:3000`
- Test data initialized

### Run E2E Tests

```bash
# Navigate to the e2e directory
cd frontend/e2e

# Run all e2e tests
npm test
# or
npx playwright test  # if using Playwright
npx cypress run      # if using Cypress
```

### Run Tests in Debug Mode

```bash
# Headless browser with debug output
npm run e2e:debug

# Interactive headed mode (browser window visible)
npx playwright test --headed
npx cypress open
```

### E2E Test Examples

**Vault Creation Test:**
```typescript
test('user can create a vault', async ({ page }) => {
  // Navigate to app
  await page.goto('http://localhost:5173');

  // Authenticate
  await page.fill('input[name="email"]', 'test@example.com');
  await page.click('button:has-text("Sign In")');

  // Create vault
  await page.click('button:has-text("New Vault")');
  await page.fill('input[name="beneficiary"]', 'beneficiary@example.com');
  await page.fill('input[name="ttl"]', '7');
  await page.click('button:has-text("Create")');

  // Verify success
  await expect(page.locator('text=Vault created successfully')).toBeVisible();
});
```

### E2E Test Best Practices

1. **Test real user workflows** - Walk through actual use cases
2. **Avoid brittle selectors** - Use semantic locators when possible
3. **Add explicit waits** - Wait for elements to load
4. **Clean up test data** - Reset between test runs
5. **Test error cases** - Verify error handling works

## Development Workflow

### 1. Create a New Component

```bash
# Create component directory
mkdir src/components/MyComponent

# Create component file
cat > src/components/MyComponent/MyComponent.tsx << 'EOF'
import React from 'react';

interface MyComponentProps {
  title: string;
}

export const MyComponent: React.FC<MyComponentProps> = ({ title }) => {
  return <div>{title}</div>;
};
EOF

# Create test file
cat > src/__tests__/MyComponent.test.tsx << 'EOF'
import { render, screen } from '@testing-library/react';
import { MyComponent } from '../components/MyComponent/MyComponent';

describe('MyComponent', () => {
  it('renders the title', () => {
    render(<MyComponent title="Test" />);
    expect(screen.getByText('Test')).toBeInTheDocument();
  });
});
EOF
```

### 2. Make Changes and Test

```bash
# Terminal 1: Run development server
npm run dev

# Terminal 2: Watch tests
npm run test:watch

# Make changes to src/components/MyComponent/MyComponent.tsx
# Watch both the browser and test output update in real-time
```

### 3. Build for Production

```bash
npm run build
```

This creates an optimized production build in the `dist/` directory.

### 4. Preview Production Build

```bash
npm run preview
```

This serves the production build locally for testing.

## Common Tasks

### Add a New Dependency

```bash
npm install package-name
# or for dev dependencies:
npm install --save-dev package-name
```

### Update All Dependencies

```bash
npm update
```

### Clear Node Modules and Reinstall

```bash
rm -rf node_modules package-lock.json
npm install
```

### Format Code (if Prettier is configured)

```bash
npm run format
# or
npx prettier --write src/
```

### Lint Code

```bash
npm run lint
```

### Check TypeScript

```bash
npx tsc --noEmit
```

## Troubleshooting

### Issue: "Cannot find module" errors

**Solution:** Run `npm install` to ensure all dependencies are installed.

```bash
cd frontend
npm install
```

### Issue: Backend connection errors

**Problem:** Frontend cannot connect to the backend API.

**Solution:** 
1. Verify the backend is running: `curl http://localhost:3000/health`
2. Check `VITE_API_URL` in `.env.local` matches your backend URL
3. Ensure CORS is enabled on the backend

### Issue: Hot module replacement not working

**Problem:** Changes to files don't trigger a refresh.

**Solution:**
1. Check the development server is running
2. Restart the dev server: `npm run dev`
3. Clear browser cache (Cmd+Shift+R on Mac, Ctrl+Shift+R on Windows)

### Issue: Tests failing with "Cannot find setup file"

**Problem:** `setupTests.ts` not found.

**Solution:**
```bash
# Create setupTests.ts if missing
touch src/setupTests.ts
```

### Issue: Port 5173 already in use

**Problem:** Another process is using the dev server port.

**Solution:**
```bash
# Kill the process using port 5173
# On macOS/Linux:
lsof -ti:5173 | xargs kill -9

# On Windows:
netstat -ano | findstr :5173
taskkill /PID <PID> /F

# Or use a different port:
npm run dev -- --port 5174
```

### Issue: "VITE_API_URL is undefined"

**Problem:** Environment variables not loading.

**Solution:**
1. Ensure `.env.local` exists in the `frontend/` directory (not root)
2. Restart the dev server for variables to load
3. Prefix all client-side variables with `VITE_`

### Issue: TypeScript errors in IDE but tests pass

**Problem:** IDE/editor not recognizing TypeScript config.

**Solution:**
1. Restart your editor/IDE
2. Run `npx tsc --noEmit` to verify configuration
3. Ensure `tsconfig.json` exists and is valid

## Additional Resources

- [Vite Documentation](https://vitejs.dev/)
- [React Documentation](https://react.dev/)
- [TypeScript Handbook](https://www.typescriptlang.org/docs/)
- [Jest Documentation](https://jestjs.io/)
- [React Testing Library](https://testing-library.com/react)
- [Project CONTRIBUTING Guide](../CONTRIBUTING.md)
