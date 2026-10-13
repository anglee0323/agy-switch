import { createBrowserRouter, Navigate } from 'react-router-dom';
import { RouterProvider } from 'react-router/dom';

import Layout from './components/layout/Layout';
import Dashboard from './pages/Dashboard';
import Accounts from './pages/Accounts';
import Settings from './pages/Settings';
import MenuBarDashboard from './pages/MenuBarDashboard';
import ThemeManager from './components/common/ThemeManager';
import { useEffect } from 'react';
import { useConfigStore } from './stores/useConfigStore';
import { useAccountStore } from './stores/useAccountStore';
import { useTranslation } from 'react-i18next';
import { listen } from '@tauri-apps/api/event';
import { isTauri } from './utils/env';
import UpdateDialog from './components/updater/UpdateDialog';
import { useUpdateStore } from './stores/useUpdateStore';
import { installMainWindowPresentation } from './utils/mainWindowPresentation';

const router = createBrowserRouter([
  { path: "/menubar", element: <MenuBarDashboard /> },
  {
    path: '/',
    element: <Layout />,
    children: [
      {
        index: true,
        element: <Dashboard />,
      },
      {
        path: 'accounts',
        element: <Accounts />,
      },
      {
        path: 'settings',
        element: <Settings />,
      },
      {
        path: '*',
        element: <Navigate to="/" replace />,
      },
    ],
  },
]);

function App() {
  const { config, loadConfig } = useConfigStore();
  const { fetchCurrentAccount, fetchAccounts } = useAccountStore();
  const { i18n } = useTranslation();

  useEffect(() => {
    loadConfig();
    if (window.location.pathname !== '/menubar') {
      fetchCurrentAccount();
      fetchAccounts();
    }
  }, [loadConfig, fetchCurrentAccount, fetchAccounts]);

  // Sync language from config
  useEffect(() => {
    if (config?.language) {
      const language = config.language.toLowerCase().startsWith('en') ? 'en' : 'zh';
      i18n.changeLanguage(language);
      document.documentElement.dir = 'ltr';
    }
  }, [config?.language, i18n]);

  // Listen for tray events
  useEffect(() => {
    if (!isTauri()) return;
    const unlistenPromises: Promise<() => void>[] = [];

    unlistenPromises.push(listen('config://updated', () => { loadConfig(); }));
    if (window.location.pathname === '/menubar') {
      return () => { Promise.all(unlistenPromises).then(listeners => listeners.forEach(unlisten => unlisten())); };
    }
    unlistenPromises.push(installMainWindowPresentation(route => router.navigate(route, { flushSync: true })));

    // 监听托盘切换账号事件
    unlistenPromises.push(
      listen('tray://account-switched', () => {
        console.log('[App] Tray account switched, refreshing...');
        fetchCurrentAccount();
        fetchAccounts();
      })
    );

    // 监听托盘刷新事件
    unlistenPromises.push(
      listen('tray://refresh-current', () => {
        console.log('[App] Tray refresh triggered, refreshing...');
        fetchCurrentAccount();
        fetchAccounts();
      })
    );

    // 监听后端全量刷新事件 (Command / Scheduler)
    unlistenPromises.push(
      listen('accounts://refreshed', () => {
        console.log('[App] Backend triggered quota refresh, syncing UI...');
        fetchCurrentAccount();
        fetchAccounts();
      })
    );

    // Cleanup
    return () => {
      Promise.all(unlistenPromises).then(unlisteners => {
        unlisteners.forEach(unlisten => unlisten());
      });
    };
  }, [fetchCurrentAccount, fetchAccounts, loadConfig]);

  // 启动时在后台静默检查一次更新 (延迟 4 秒，避开应用启动峰值)
  useEffect(() => {
    if (!isTauri() || window.location.pathname === '/menubar' || !config || config.check_updates_on_startup === false) return;
    const timer = setTimeout(() => {
      void useUpdateStore.getState().checkForUpdates(true);
    }, 4000);
    return () => clearTimeout(timer);
  }, [Boolean(config), config?.check_updates_on_startup]);

  return (
    <>
      <ThemeManager />
      <RouterProvider router={router} />
      <UpdateDialog />
    </>
  );
}

export default App;
