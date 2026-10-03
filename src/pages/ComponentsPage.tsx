import { ComponentCenter } from "../ComponentCenter";
import type {
  ComponentBusyState,
  ComponentManifest,
  ComponentOperation,
} from "../types";

type ComponentsPageProps = {
  components: ComponentManifest[];
  busyState: Record<string, ComponentBusyState>;
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
  return (
    <div className="page-stack">
      <ComponentCenter
        items={components}
        busyState={busyState}
        captureHelperEnabled={captureHelperEnabled}
        onToggleCaptureHelper={onToggleCaptureHelper}
        onManage={onManageComponent}
        onLaunch={onLaunchComponent}
        onOpenTarget={onOpenTarget}
      />
    </div>
  );
}
