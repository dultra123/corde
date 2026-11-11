import type { WidgetPlacement } from "@corde/protocol";
import type { GridStack } from "gridstack";
import { Component, type ReactNode, useCallback, useLayoutEffect, useRef } from "react";
import { useT } from "../i18n";
import { useDashboard } from "../store/dashboard";
import { getWidget } from "../widgets";
import type { WidgetDef } from "../widgets/registry";

interface Props {
  grid: GridStack;
  placement: WidgetPlacement;
  editing: boolean;
}

class WidgetBoundary extends Component<
  { children: ReactNode; label: string },
  { error: string | null }
> {
  state = { error: null as string | null };
  static getDerivedStateFromError(err: unknown) {
    return { error: err instanceof Error ? err.message : String(err) };
  }
  render() {
    if (this.state.error !== null) {
      return (
        <div className="w-empty is-error">
          <span>
            {this.props.label}
            <br />
            <small>{this.state.error}</small>
          </span>
        </div>
      );
    }
    return this.props.children;
  }
}

function ConfigFields({ def, placement }: { def: WidgetDef; placement: WidgetPlacement }) {
  const { t } = useT();
  const setWidgetConfig = useDashboard((s) => s.setWidgetConfig);
  if (!def.configFields?.length) return null;
  return (
    <span className="widget-config">
      {def.configFields.map((f) => {
        const cur = placement.config[f.key] ?? def.defaultConfig?.[f.key];
        return (
          <select
            key={f.key}
            className="widget-config-select"
            value={cur === undefined ? "" : String(cur)}
            aria-label={t(f.labelKey)}
            onChange={(e) => setWidgetConfig(placement.id, { [f.key]: e.target.value })}
          >
            {f.options.map((o) => (
              <option key={o.value} value={o.value}>
                {t(o.labelKey)}
              </option>
            ))}
          </select>
        );
      })}
    </span>
  );
}

export function GridItem({ grid, placement, editing }: Props) {
  const { t } = useT();
  const ref = useRef<HTMLDivElement>(null);
  const removeWidget = useDashboard((s) => s.removeWidget);
  const setWidgetConfig = useDashboard((s) => s.setWidgetConfig);
  const def = getWidget(placement.kind);

  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    grid.makeWidget(el);
    return () => {
      try {
        grid.removeWidget(el, false, false);
      } catch {
        // grid already destroyed (layout switch): nothing to detach
      }
    };
  }, [grid]);

  const setConfig = useCallback(
    (patch: Record<string, unknown>) => setWidgetConfig(placement.id, patch),
    [placement.id, setWidgetConfig],
  );

  const title = def ? t(def.titleKey) : `${t("widget.unknown")} : ${placement.kind}`;
  const Comp = def?.component;

  return (
    <div
      ref={ref}
      className="grid-stack-item"
      gs-id={placement.id}
      gs-x={placement.x}
      gs-y={placement.y}
      gs-w={placement.w}
      gs-h={placement.h}
      gs-min-w={def?.minW ?? 1}
      gs-min-h={def?.minH ?? 1}
    >
      <div className="grid-stack-item-content">
        <section className={`widget is-${placement.kind}`} aria-label={title}>
          <header className="widget-head">
            <span className="widget-title">{title}</span>
            {editing && def && <ConfigFields def={def} placement={placement} />}
            {editing && (
              <button
                type="button"
                className="widget-remove"
                aria-label={t("widget.remove")}
                title={t("widget.remove")}
                onPointerDown={(e) => e.stopPropagation()}
                onClick={() => removeWidget(placement.id)}
              >
                ×
              </button>
            )}
          </header>
          <div className="widget-body">
            {Comp ? (
              <WidgetBoundary label={title}>
                <Comp
                  id={placement.id}
                  config={placement.config}
                  setConfig={setConfig}
                  editing={editing}
                />
              </WidgetBoundary>
            ) : (
              <div className="w-empty">
                <span>{t("widget.unknownHint")}</span>
              </div>
            )}
          </div>
        </section>
      </div>
    </div>
  );
}
