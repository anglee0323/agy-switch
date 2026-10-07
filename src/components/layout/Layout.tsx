import { Outlet } from 'react-router-dom';
import { getCurrentWindow } from '@tauri-apps/api/window';
import Navbar from '../navbar/Navbar';
import { AutoSwitchStatusBar } from '../autoSwitch/AutoSwitch';
import ToastContainer from '../common/ToastContainer';
import { useEffect } from 'react';
import { isTauri } from '../../utils/env';
import { ensureFullViewState } from '../../utils/windowManager';

function Layout() {
    useEffect(() => {
        if (isTauri()) {
            ensureFullViewState();
        }
    }, []);

    return (
        <div className="h-screen flex flex-col bg-[#FAFBFC] dark:bg-base-300">
            {/* 全局窗口拖拽区域 - 使用 JS 手动触发拖拽，解决 HTML 属性失效问题 */}
            <div
                className="fixed top-0 left-0 right-0 h-9"
                style={{
                    zIndex: 9999,
                    backgroundColor: 'rgba(0,0,0,0.001)',
                    cursor: 'default',
                    userSelect: 'none',
                    WebkitUserSelect: 'none'
                }}
                data-tauri-drag-region
                onMouseDown={() => {
                    getCurrentWindow().startDragging();
                }}
            />
            <ToastContainer />
            <Navbar />
            <AutoSwitchStatusBar />
            <main className="flex-1 overflow-hidden flex flex-col relative">
                <Outlet />
            </main>
        </div>
    );
}

export default Layout;
