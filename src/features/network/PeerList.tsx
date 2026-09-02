/**
 * Who this node is connected to, one row each — or the reason there is nobody.
 *
 * # The count was all there was, and now there is a list
 *
 * The mesh kept a set of peer identifiers and handed over its size, so this screen could say *3*
 * and nothing else. It keeps the address each connection is on now (`almena_mesh::Peers::reached`),
 * so each peer is a row: **what it answers to on the mesh**, and **where this node is talking to
 * it**. Both are facts about sockets and neither is written down anywhere (`SPECS.md §17.18`) —
 * where a peer was reached in fact stays with the node that reached it.
 *
 * **And how far away it is**, which is the one thing about a connection this node goes and finds
 * out rather than being told: libp2p's ping runs for as long as a peer is connected and the mesh
 * keeps the last round trip. It is the last and never an average — what a person reads a latency
 * for is *how far away is it now*, and a mean over an hour hides the minute it went bad.
 *
 * A peer that has just arrived has no round trip yet and says so rather than showing a nought,
 * which would be the fastest connection on the list, invented.
 *
 * **What is still not here is what nobody measured.** The software on the other end and which
 * streams are open are things a node application usually draws beside a peer, and this node asks
 * for neither. A column of blanks would be a promise; where they would come from, if they ever
 * do, is the mesh and not this file.
 *
 * # Four emptinesses, and they are not interchangeable
 *
 * - **Nobody has looked yet.** Before the first reading, saying *no peers* reports a result nobody
 *   obtained. That it is true for only a moment is not a defence.
 * - **There is no network.** Nothing was joined, so there was never anything to count.
 * - **There is a network and no place on the mesh.** The node holds its record and answers for it
 *   and has not taken a port — the state a start leaves when the port it remembers is taken.
 * - **There is a mesh and nobody on the other end.** Somebody counted, and the answer is none.
 *
 * # The filter is on the rows and not on the node
 *
 * Typing narrows what is drawn and asks the node nothing: the list is already here, and a filter
 * that went back to the mesh would make a keystroke a network round trip. The count beside the box
 * is **what is showing**, so a filter that matches nothing says nothing is showing rather than
 * appearing to have lost the peers.
 */

import { useMemo, useState } from "react";
import { Radio, RadioTower, Unplug, Users } from "lucide-react";
import { useTranslation } from "react-i18next";

import EmptyState from "@/components/EmptyState";
import Copyable from "@/components/Copyable";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { transportOf, type NetworkReading, type Peer } from "@/lib/network";

/** What the list is drawn from. */
interface PeerListProps {
  /** The last reading, or `null` when none has come back yet. */
  reading: NetworkReading | null;
  /**
   * Who is connected, `null` where nobody counted, or `undefined` before the first look.
   *
   * Three values and not two: a node with no place on the mesh has counted nobody, and before the
   * first look nobody has even asked. They are drawn differently and so they are held differently.
   */
  peers: Peer[] | null | undefined;
}

/** Who this node is connected to. */
function PeerList({ reading, peers }: PeerListProps) {
  const { t } = useTranslation();
  const [filter, setFilter] = useState("");

  /* Recomputed only when the list or the text changes, because it runs on every keystroke over a
     list that can be hundreds long. Matched against both columns: somebody pasting an address is
     looking for the same row as somebody pasting an identifier. */
  const showing = useMemo(() => {
    const wanted = filter.trim().toLowerCase();
    if (peers == null) return [];
    if (wanted === "") return peers;
    return peers.filter(
      (peer) =>
        peer.peer.toLowerCase().includes(wanted) ||
        peer.address.toLowerCase().includes(wanted),
    );
  }, [peers, filter]);

  if (reading === null || peers === undefined) {
    return (
      <EmptyState
        icon={<Radio aria-hidden="true" />}
        title={t("network.peers.lookingTitle")}
        reason={t("network.peers.looking")}
      />
    );
  }

  if (peers === null) {
    return reading.network === null ? (
      <EmptyState
        icon={<Unplug aria-hidden="true" />}
        title={t("network.peers.noNetworkTitle")}
        reason={t("network.peers.noNetwork")}
      />
    ) : (
      <EmptyState
        icon={<RadioTower aria-hidden="true" />}
        title={t("network.peers.noMeshTitle")}
        reason={t("network.peers.noMesh")}
      />
    );
  }

  if (peers.length === 0) {
    return (
      <EmptyState
        icon={<Users aria-hidden="true" />}
        title={t("network.peers.noneTitle")}
        reason={t("network.peers.none")}
      />
    );
  }

  return (
    <div className="flex flex-col gap-4">
      {/* **The count, at the size of the thing somebody came to see.** It is the one figure on
          this screen a person reads from across the room, and it is what is showing rather than
          what exists — the two differ the moment anything is typed below. */}
      <div className="flex flex-wrap items-baseline gap-3">
        <span className="text-3xl font-semibold tabular-nums">{showing.length}</span>
        <span className="text-muted-foreground text-sm tracking-wide uppercase">
          {t("network.peers.counted", { count: showing.length })}
        </span>
      </div>

      <input
        className="border-input bg-transparent h-9 w-full rounded-md border px-3 text-sm"
        aria-label={t("network.peers.filter")}
        placeholder={t("network.peers.filter")}
        value={filter}
        onChange={(event: React.ChangeEvent<HTMLInputElement>) => setFilter(event.target.value)}
      />

      {showing.length === 0 ? (
        /* Not one of the four emptinesses above: there are peers and this filter matches none of
           them, which is a fact about what was typed and is undone by clearing it. */
        <EmptyState
          icon={<Users aria-hidden="true" />}
          title={t("network.peers.noMatchTitle")}
          reason={t("network.peers.noMatch")}
        />
      ) : (
        /* The one thing on this screen wide enough to need it: a peer identifier and a
           multiaddress side by side outrun a narrow window, and a table that wrapped them would
           be unreadable in both columns. It scrolls inside its own box so that the screen behind
           it never does. */
        <div className="w-full overflow-x-auto">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t("network.peers.column.peer")}</TableHead>
                <TableHead>{t("network.peers.column.connection")}</TableHead>
                <TableHead className="text-right">
                  {t("network.peers.column.far")}
                </TableHead>
                <TableHead>{t("network.peers.column.address")}</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {showing.map((peer) => (
                <TableRow key={peer.peer}>
                  <TableCell className="font-mono">
                    <Copyable value={peer.peer} what={t("network.peers.column.peer")} />
                  </TableCell>
                  {/* Read off the address beside it and never asked for: what the multiaddress
                      says about which family over which transport. Empty where this build knows
                      none of its parts, which is the honest answer to not being able to say. */}
                  <TableCell className="text-muted-foreground font-mono">
                    {transportOf(peer.address)}
                  </TableCell>
                  {/* Right-aligned and tabular, because a column of numbers is read down its
                      last digit. Absent says so in words: a dash alone tells a screen reader
                      nothing about the difference between nought and nobody looked. */}
                  <TableCell className="text-right font-mono tabular-nums">
                    {peer.far === null ? (
                      <span className="text-faint">
                        <span aria-hidden="true">—</span>
                        <span className="sr-only">{t("network.peers.unmeasuredFar")}</span>
                      </span>
                    ) : (
                      t("network.peers.ms", { ms: peer.far })
                    )}
                  </TableCell>
                  <TableCell className="font-mono">
                    <Copyable value={peer.address} what={t("network.peers.column.address")} />
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}
    </div>
  );
}

export default PeerList;
