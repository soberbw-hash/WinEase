import { useEffect, useRef, useState } from "react";
import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AppUpdater } from "./AppUpdater";
import { BossModeOverlay } from "./BossModeOverlay";
import { homeQuickActions, sections } from "./content";
import { getBossModeViewState } from "./fakeUpdate";
import { InfoDrawer } from "./InfoDrawer";
import { useAppSettings } from "./hooks/useAppSettings";
import { CleaningPage } from "./pages/CleaningPage";
import { ComponentsPage } from "./pages/ComponentsPage";
import { FilesPage } from "./pages/FilesPage";
import { HealthPage } from "./pages/HealthPage";
import { NetworkPage } from "./pages/NetworkPage";
import { ManagementPage, type ManagementTab } from "./pages/ManagementPage";
import { HomePage } from "./pages/HomePage";
import { SettingsPage } from "./pages/SettingsPage";
import { SystemPage } from "./pages/SystemPage";
import { SupportModal } from "./SupportModal";
import { ConfirmDialog } from "./ConfirmDialog";
import type {
  ActionId,
  BossModeViewState,
  ComponentBusyState,
  ComponentManifest,
  ComponentOperation,
  SectionId,
  SystemSnapshot,
  ThirdPartyNotice,
  ToolActionResult,
} from "./types";
import "./App.css";

function App() {
  const { settings, updateSettings } = useAppSettings();
  const [activeSection, setActiveSection] = useState<SectionId>("home");
  const [snapshot, setSnapshot] = useState<SystemSnapshot | null>(null);
  const [snapshotError, setSnapshotError] = useState("");
  const [components, setComponents] = useState<ComponentManifest[]>([]);
  const [componentError, setComponentError] = useState("");
  const [notices, setNotices] = useState<ThirdPartyNotice[]>([]);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [supportOpen, setSupportOpen] = useState(false);
  const [managementTab, setManagementTab] =
    useState<ManagementTab>("processes");
  const [duplicates, setDuplicates] = useState(false);
  const [fileNavigation, setFileNavigation] = useState(0);
  const [history, setHistory] = useState<ToolActionResult[]>([]);
  const [, setRunningActionId] = useState<string | null>(null);
  const [componentBusy, setComponentBusy] = useState<ComponentBusyState | null>(
    null,
  );
  const [pendingUninstall, setPendingUninstall] = useState<string | null>(null);
  const [toast, setToast] = useState<{
    message: string;
    error: boolean;
  } | null>(null);
  const [bossMode, setBossMode] = useState(false);
  const [bossStartedAt, setBossStartedAt] = useState(0);
  const [bossState, setBossState] = useState<BossModeViewState>(
    getBossModeViewState(0),
  );
  const actionLock = useRef(false),
    componentLock = useRef(false),
    snapshotLock = useRef(false);
  const bossTransition = useRef(false);
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const capturePlus = components.find((item) => item.id === "capture-plus");

  function pushToast(message: string, error = false) {
    if (toastTimer.current) clearTimeout(toastTimer.current);
    setToast({ message, error });
    toastTimer.current = setTimeout(() => setToast(null), 4000);
  }
  function recordResult(result: ToolActionResult) {
    setHistory((current) => [result, ...current].slice(0, 20));
    pushToast(result.summary, !result.success);
  }
  async function loadSnapshot() {
    if (snapshotLock.current) return;
    snapshotLock.current = true;
    try {
      setSnapshot(await invoke<SystemSnapshot>("get_system_snapshot"));
      setSnapshotError("");
    } catch (error) {
      setSnapshotError(String(error));
    } finally {
      snapshotLock.current = false;
    }
  }
  async function loadComponents() {
    try {
      const items = await invoke<ComponentManifest[]>("list_components");
      const nextNotices: ThirdPartyNotice[] = items
        .filter((item) => item.sourceUrl && item.licenseName)
        .map((item) => ({
          id: item.id,
          name: item.name,
          version: item.version ?? "跟随安装源",
          sourceLabel: item.sourceLabel ?? "官方",
          sourceUrl: item.sourceUrl!,
          licenseName: item.licenseName!,
          licenseUrl: item.licenseUrl,
          notes: "",
        }));
      setComponents(items);
      setNotices(nextNotices);
      setComponentError("");
      if (!items.some((item) => item.id === "capture-plus" && item.installed)) {
        updateSettings({ captureHelperEnabled: false });
      }
    } catch (error) {
      setComponentError(String(error));
    }
  }
  async function runAction(actionId: ActionId) {
    if (actionLock.current) return;
    actionLock.current = true;
    setRunningActionId(actionId);
    try {
      recordResult(
        await invoke<ToolActionResult>("run_tool_action", {
          actionId,
          captureHelperEnabled: settings.captureHelperEnabled,
        }),
      );
    } catch (error) {
      recordResult({
        actionId,
        title: "操作失败",
        success: false,
        summary: String(error),
        details: String(error),
        durationMs: 0,
        warnings: [],
      });
    } finally {
      actionLock.current = false;
      setRunningActionId(null);
    }
  }
  async function manageComponent(
    componentId: string,
    operation: ComponentOperation | "launch",
  ) {
    if (componentLock.current) return false;
    componentLock.current = true;
    const labels = {
      install: "正在安装",
      repair: "正在修复",
      uninstall: "正在卸载",
      disable: "正在关闭",
      update: "正在更新",
      launch: "正在打开",
    };
    setComponentBusy({
      componentId,
      operation,
      progress: 0,
      stageLabel: labels[operation],
    });
    try {
      const result = await invoke<ToolActionResult>(
        operation === "launch" ? "launch_component" : "manage_component",
        { componentId, operation },
      );
      recordResult(result);
      if (operation !== "launch") await loadComponents();
      return result.success;
    } catch (error) {
      pushToast(String(error), true);
      return false;
    } finally {
      componentLock.current = false;
      setComponentBusy(null);
    }
  }
  function requestComponentOperation(
    componentId: string,
    operation: ComponentOperation,
  ) {
    if (operation === "uninstall") setPendingUninstall(componentId);
    else void manageComponent(componentId, operation);
  }
  async function toggleCaptureHelper(enabled: boolean) {
    if (enabled) {
      if (
        !capturePlus?.installed &&
        !(await manageComponent("capture-plus", "install"))
      )
        return;
      if (await manageComponent("capture-plus", "launch"))
        updateSettings({ captureHelperEnabled: true });
    } else if (await manageComponent("capture-plus", "disable"))
      updateSettings({ captureHelperEnabled: false });
  }
  async function openTarget(target: string) {
    try {
      const result = await invoke<ToolActionResult>("open_target", { target });
      if (!result.success) pushToast(result.summary, true);
    } catch (error) {
      pushToast(String(error), true);
    }
  }
  function quickAction(id: string) {
    if (id === "open_cleaning") setActiveSection("cleaning");
    else if (id === "open_storage" || id === "open_duplicates") {
      setDuplicates(id === "open_duplicates");
      setFileNavigation((value) => value + 1);
      setActiveSection("efficiency");
    } else if (id === "open_health") setActiveSection("health");
    else if (id === "open_network") setActiveSection("network");
    else if (
      [
        "open_startup",
        "open_processes",
        "open_uninstall",
        "open_popups",
      ].includes(id)
    ) {
      setManagementTab(
        (
          {
            open_startup: "startup",
            open_processes: "processes",
            open_uninstall: "uninstall",
            open_popups: "popups",
          } as Record<string, ManagementTab>
        )[id],
      );
      setActiveSection("applications");
    } else if (id === "open_system") setActiveSection("system");
    else void runAction(id as ActionId);
  }
  async function toggleBossMode(enabled: boolean) {
    if (bossTransition.current) return;
    bossTransition.current = true;
    try {
      if (enabled) {
        setBossMode(true);
        setBossStartedAt(Date.now());
        setBossState(getBossModeViewState(0));
        await getCurrentWindow().setDecorations(false);
        await getCurrentWindow().setFullscreen(true);
        await getCurrentWindow().setAlwaysOnTop(true);
        await getCurrentWindow().setFocus();
      } else {
        await getCurrentWindow().setAlwaysOnTop(false);
        await getCurrentWindow().setFullscreen(false);
        await getCurrentWindow().setDecorations(true);
        setBossMode(false);
      }
    } catch (error) {
      await Promise.allSettled([
        getCurrentWindow().setAlwaysOnTop(false),
        getCurrentWindow().setFullscreen(false),
        getCurrentWindow().setDecorations(true),
      ]);
      setBossMode(false);
      pushToast(String(error), true);
    } finally {
      bossTransition.current = false;
    }
  }
  useEffect(() => {
    if (isTauri()) {
      void loadComponents();
    }
    return () => {
      if (toastTimer.current) clearTimeout(toastTimer.current);
    };
  }, []);
  useEffect(() => {
    let cancelled = false,
      checking = false;
    async function inspect() {
      if (!isTauri() || checking) return;
      checking = true;
      try {
        const rows = await invoke<
          Array<{ id: string; available: boolean; version: string | null }>
        >("check_component_updates");
        if (!cancelled)
          setComponents((items) =>
            items.map((item) => {
              const row = rows.find((row) => row.id === item.id);
              return row
                ? {
                    ...item,
                    updateAvailable: row.available,
                    availableVersion: row.version,
                  }
                : item;
            }),
          );
      } catch {
        /* A failed background check must not mark packages current. */
      } finally {
        checking = false;
      }
    }
    const first = setTimeout(() => void inspect(), 30000);
    const timer = setInterval(() => void inspect(), 6 * 60 * 60 * 1000);
    return () => {
      cancelled = true;
      clearTimeout(first);
      clearInterval(timer);
    };
  }, []);
  useEffect(() => {
    if (activeSection === "settings" && isTauri()) void loadSnapshot();
  }, [activeSection]);
  useEffect(() => {
    const timer = setInterval(() => {
      if (
        isTauri() &&
        activeSection === "settings" &&
        !document.hidden &&
        !bossMode
      )
        void loadSnapshot();
    }, 60_000);
    return () => clearInterval(timer);
  }, [bossMode, activeSection]);
  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (
        (event.ctrlKey && event.altKey && event.key.toLowerCase() === "b") ||
        (bossMode && event.key === "Escape")
      ) {
        event.preventDefault();
        void toggleBossMode(!bossMode);
      }
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [bossMode]);
  useEffect(() => {
    if (!bossMode) return;
    const timer = setInterval(
      () => setBossState(getBossModeViewState(Date.now() - bossStartedAt)),
      250,
    );
    return () => clearInterval(timer);
  }, [bossMode, bossStartedAt]);
  function renderPage() {
    switch (activeSection) {
      case "home":
        return (
          <HomePage
            quickActions={homeQuickActions}
            onQuickAction={quickAction}
          />
        );
      case "cleaning":
        return <CleaningPage onResult={recordResult} />;
      case "health":
        return <HealthPage onNavigate={quickAction} />;
      case "network":
        return (
          <NetworkPage onOpenTarget={(target) => void openTarget(target)} />
        );
      case "applications":
        return (
          <ManagementPage
            initialTab={managementTab}
            components={components}
            componentBusy={componentBusy !== null}
            onComponent={(id, installed) => {
              void (async () => {
                if (installed || (await manageComponent(id, "install")))
                  await manageComponent(id, "launch");
              })();
            }}
            onOpenTarget={(target) => void openTarget(target)}
          />
        );
      case "system":
        return (
          <SystemPage onOpenTarget={(target) => void openTarget(target)} />
        );
      case "components":
        return (
          <ComponentsPage
            components={components}
            busyState={componentBusy}
            captureHelperEnabled={settings.captureHelperEnabled}
            onToggleCaptureHelper={(enabled) =>
              void toggleCaptureHelper(enabled)
            }
            onManageComponent={requestComponentOperation}
            onLaunchComponent={(id) => void manageComponent(id, "launch")}
            onOpenTarget={(target) => void openTarget(target)}
          />
        );
      case "efficiency":
        return null;
      case "settings":
        return (
          <SettingsPage
            snapshot={snapshot}
            onOpenDeviceInfo={() => setDrawerOpen(true)}
            components={components}
            notices={notices}
            busyState={componentBusy}
            onEnterBossMode={() => void toggleBossMode(true)}
            onManageComponent={requestComponentOperation}
            onLaunchComponent={(id) => void manageComponent(id, "launch")}
            onOpenTarget={(target) => void openTarget(target)}
            onOpenSupportModal={() => setSupportOpen(true)}
          />
        );
    }
  }
  return (
    <>
      <div className="app-shell">
        <aside className="sidebar">
          <div className="sidebar__brand">
            <img src="/brand-icon.png" alt="" className="sidebar__logo-image" />
            <div>
              <h1>WinEase</h1>
              <p className="sidebar__product-label">Windows 工具箱</p>
            </div>
          </div>
          <nav className="sidebar__nav" aria-label="主导航">
            {sections.map((section) => (
              <button
                key={section.id}
                type="button"
                aria-current={activeSection === section.id ? "page" : undefined}
                className={`sidebar__nav-item ${activeSection === section.id ? "sidebar__nav-item--active" : ""}`}
                onClick={() => setActiveSection(section.id)}
              >
                <strong>{section.label}</strong>
              </button>
            ))}
          </nav>
        </aside>
        <main className="main-panel">
          <header className="main-toolbar">
            <h2>
              {sections.find((item) => item.id === activeSection)?.label ??
                (activeSection === "health" ? "电脑体检" : "网络修复")}
            </h2>
            <div className="main-toolbar__actions">
              {(activeSection === "health" || activeSection === "network") && (
                <button
                  className="ghost-button"
                  onClick={() => setActiveSection("home")}
                >
                  返回首页
                </button>
              )}
            </div>
          </header>
          <div className="page-frame">
            {snapshotError && activeSection === "settings" && (
              <div className="banner banner--warning" role="alert">
                系统信息读取失败：{snapshotError}
              </div>
            )}
            {componentError &&
              (activeSection === "components" ||
                activeSection === "settings") && (
                <div className="banner banner--warning" role="alert">
                  <span>组件读取失败：{componentError}</span>
                  <button
                    className="ghost-button"
                    onClick={() => void loadComponents()}
                  >
                    重试
                  </button>
                </div>
              )}
            {renderPage()}
            <div hidden={activeSection !== "efficiency"}>
              <FilesPage
                navigationRequest={fileNavigation}
                initialDuplicates={duplicates}
                onOpenTarget={(target) => void openTarget(target)}
              />
            </div>
          </div>
        </main>
      </div>
      <InfoDrawer
        open={drawerOpen}
        snapshot={snapshot}
        history={history}
        onClose={() => setDrawerOpen(false)}
        onOpenTarget={(target) => void openTarget(target)}
      />
      <AppUpdater />
      <SupportModal open={supportOpen} onClose={() => setSupportOpen(false)} />
      <ConfirmDialog
        open={pendingUninstall !== null}
        title={`卸载 ${components.find((item) => item.id === pendingUninstall)?.name ?? "组件"}？`}
        description="将调用组件卸载程序。"
        confirmLabel="卸载"
        onCancel={() => setPendingUninstall(null)}
        onConfirm={() => {
          const id = pendingUninstall;
          setPendingUninstall(null);
          if (id) void manageComponent(id, "uninstall");
        }}
      />
      {bossMode && <BossModeOverlay state={bossState} />}
      {toast && !bossMode && (
        <div
          role="status"
          className={`toast toast--${toast.error ? "error" : "info"}`}
        >
          {toast.message}
        </div>
      )}
    </>
  );
}
export default App;
