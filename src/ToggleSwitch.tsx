export function ToggleSwitch({label, checked, disabled = false, state, onChange}: {
  label: string; checked: boolean; disabled?: boolean; state?: string; onChange: (checked: boolean) => void;
}) {
  return <label className="startup-toggle">
    <span>{state ?? (checked ? "开" : "关")}</span>
    <input type="checkbox" role="switch" aria-label={label} checked={checked} disabled={disabled}
      onChange={event => onChange(event.target.checked)}/>
    <span className="toggle-track" aria-hidden="true"/>
  </label>;
}
