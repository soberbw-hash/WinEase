import { ComponentCenter } from "../ComponentCenter";
import type {
  ComponentBusyState,
  ComponentManifest,
  ComponentOperation,
} from "../types";

type ComponentsPageProps = {
  components: ComponentManifest[];
  busyState: ComponentBusyState | null;
  captureHelperEnabled: boolean;
  onToggleCaptureHelper: (nextEnabled: boolean) => void;
  onManageComponent: (
    componentId: string,
    operation: ComponentOperation,
  ) => void;
  onLaunchComponent: (componentId: string) => void;
  onOpenTarget: (target: string) => void;
};

export function ComponentsPage({
  components,
  busyState,
  captureHelperEnabled,
  onToggleCaptureHelper,
  onManageComponent,
  onLaunchComponent,
  onOpenTarget,
}: ComponentsPageProps) {
  const capturePlus = components.find((item) => item.id === "capture-plus");
  const captureBusy = busyState?.componentId === "capture-plus";

  const captureStatus = capturePlus?.installed
    ? captureHelperEnabled
      ? "F1 截图 · F3 贴图"
      : "已安装，未启用"
    : "启用时安装 Snipaste";

  return (
    <div className="page-stack">
      <section className="surface">
        <div className="setting-line">
          <div>
            <h3>Snipaste 截图增强</h3>
            <p className="scope-note">{captureStatus}</p>
          </div>
          <div className="button-row">
            <label>
              <input
                type="checkbox"
                checked={captureHelperEnabled}
                disabled={captureBusy || busyState !== null}
                onChange={(e) => onToggleCaptureHelper(e.target.checked)}
              />
              启用
            </label>
            <button
              className="secondary-button"
              onClick={() => onOpenTarget("ms-screenclip:")}
            >
              截图
            </button>
          </div>
        </div>
      </section>
      <ComponentCenter
        items={components}
        busyState={busyState}
        hiddenIds={[]}
        onManage={onManageComponent}
        onLaunch={onLaunchComponent}
        onOpenTarget={onOpenTarget}
      />
    </div>
  );
}
