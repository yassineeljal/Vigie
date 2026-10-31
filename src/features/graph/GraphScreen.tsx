import { useMemo } from "react";
import { useQuery } from "@tanstack/react-query";
import {
  Background,
  Controls,
  MarkerType,
  Position,
  ReactFlow,
  type Edge,
  type Node,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import { commands, type GraphData, type GraphNode } from "../../lib/ipc";
import { unwrap } from "../../lib/result";
import { Empty, PageHeader } from "../../components/ui";
import { CRITICALITY_LABELS } from "../../lib/labels";
import { t } from "../../i18n";

const COL = 300;
const ROW = 90;

function borderFor(n: GraphNode): string {
  if (n.single_point_of_failure) return "var(--danger)";
  if (n.pivot) return "var(--warn)";
  return "var(--line)";
}

function describe(n: GraphNode): string {
  const parts = [n.label];
  if (n.kind === "unlinked_identity") parts.push(t("identité hors inventaire"));
  if (n.criticality)
    parts.push(
      t("criticité {level}", { level: t(CRITICALITY_LABELS[n.criticality]).toLowerCase() }),
    );
  if (n.dependents > 0) parts.push(t("{n} compte(s) en dépendent", { n: n.dependents }));
  if (n.pivot) parts.push(t("compte pivot"));
  if (n.single_point_of_failure) parts.push(t("point de défaillance unique"));
  return parts.join(", ");
}

export function layout(data: GraphData): { nodes: Node[]; edges: Edge[] } {
  const perLevel = new Map<number, number>();
  const nodes: Node[] = data.nodes.map((n) => {
    const row = perLevel.get(n.level) ?? 0;
    perLevel.set(n.level, row + 1);
    return {
      id: n.id,
      position: { x: n.level * COL, y: row * ROW },
      // La disposition va de gauche à droite : les flèches aussi.
      sourcePosition: Position.Right,
      targetPosition: Position.Left,
      data: { label: `${n.label}${n.dependents ? ` (${n.dependents})` : ""}` },
      ariaLabel: describe(n),
      style: {
        background: n.kind === "unlinked_identity" ? "var(--sunken)" : "var(--surface)",
        color: "var(--ink)",
        border: `2px ${n.kind === "unlinked_identity" ? "dashed" : "solid"} ${borderFor(n)}`,
        borderRadius: 12,
        padding: 10,
        width: 220,
        fontSize: 13,
      },
    };
  });
  const edges: Edge[] = data.edges.map((e) => ({
    id: `${e.from}-${e.to}`,
    source: e.from,
    target: e.to,
    label: e.via === "recovery" ? "récupération" : "connexion",
    markerEnd: { type: MarkerType.ArrowClosed },
    style: { stroke: "var(--muted)" },
    labelStyle: { fill: "var(--muted)", fontSize: 11 },
    labelBgStyle: { fill: "var(--bg)" },
  }));
  return { nodes, edges };
}

export function GraphScreen() {
  const { data } = useQuery({
    queryKey: ["graph"],
    queryFn: () => unwrap(commands.getRecoveryGraph()),
  });
  const flow = useMemo(() => (data ? layout(data) : null), [data]);
  if (!data || !flow) return null;

  const critical = data.nodes.filter((n) => n.pivot || n.single_point_of_failure);
  return (
    <section className="flex h-full flex-col gap-4">
      <PageHeader title={t("Graphe de récupération")}>
        {t(
          "Une flèche va d'un compte vers celui dont il dépend pour être récupéré (ex. ton courriel). Si le nœud de droite tombe, tout ce qui pointe vers lui tombe aussi.",
        )}{" "}
        <span className="text-warn">{t("Orange : compte pivot")}</span> ·{" "}
        <span className="text-danger">{t("Rouge : point de défaillance unique")}</span>{" "}
        {t("· Pointillés : identité hors inventaire.")}
      </PageHeader>
      {data.nodes.length === 0 ? (
        <Empty>
          {t(
            "Aucune dépendance à afficher. Renseigne les identités de connexion/récupération de tes comptes et lie chaque courriel à son compte (onglet Identités).",
          )}
        </Empty>
      ) : (
        <>
          <div
            className="min-h-0 flex-1 rounded-xl border border-line"
            aria-label={t("Graphe interactif")}
          >
            <ReactFlow
              nodes={flow.nodes}
              edges={flow.edges}
              fitView
              colorMode="system"
              nodesConnectable={false}
            >
              <Background />
              <Controls showInteractive={false} />
            </ReactFlow>
          </div>
          <div aria-label={t("Alternative textuelle")}>
            <h3 className="text-sm font-medium text-ink">{t("Points sensibles")}</h3>
            {critical.length === 0 ? (
              <p className="text-sm text-safe">
                {t("Aucun pivot ni point de défaillance unique détecté.")}
              </p>
            ) : (
              <ul className="list-inside list-disc text-sm text-ink">
                {critical.map((n) => (
                  <li key={n.id}>{describe(n)}</li>
                ))}
              </ul>
            )}
          </div>
        </>
      )}
    </section>
  );
}
