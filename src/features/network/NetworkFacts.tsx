/**
 * The head of the Network screen: what is known about the network this node is on.
 *
 * Every figure is a dash until the node is on a network, and that is the point rather than a
 * placeholder: a node on no network has looked at nothing, and `Figure` is the element that draws
 * the difference between "none" and "nobody looked". The peer count stays a dash a little longer
 * — until the node has taken its place on the mesh — because until then nobody has counted.
 *
 * The same figures the terminal draws, from the same place. Neither face works a figure out.
 *
 * # What the node is doing, and why it is not
 *
 * The badge and the sentence beside it came from the card of controls that used to sit under this
 * one. That card is gone, and this is where they belong anyway: **what the node is doing is a fact
 * about the node**, which is what this card is for, and it was only ever down there because that
 * was where somebody would go to do something about it.
 *
 * It matters more than it looks. A start that could not join a network leaves the node *failing*
 * with an identifier saying which half of it failed, and the strip along the bottom has room for
 * the word alone — so **this is the only place in the window that says why**. What comes across is
 * never a sentence: the node has no idea what language anybody reads in, and two operators
 * comparing notes need the same word, so the identifier travels and the reading happens here.
 *
 * # The names first, then what was counted
 *
 * Seven figures of two kinds. Four of them are names — the network's, this node's, the root
 * over what it wrote down, and what it answers to on the mesh — and each is between forty and
 * sixty-two characters of base32; three of them are counts, and none is longer than a few
 * digits. Mixed together in one grid of equal columns they were unreadable in both directions:
 * a name painted straight over the three figures beside it, and at 400 points across it took
 * the whole window sideways.
 *
 * So the names come first and each takes the width of the card, and the counts follow in one
 * row of columns. That is `Figure`'s `identifier`, which also gives each name the button that
 * puts the whole of it on the clipboard — a name is the sort of thing somebody has to paste
 * somewhere, and a count is not.
 */

import { useTranslation } from "react-i18next";

import Figure from "@/components/Figure";
import StateBadge, { type StateTone } from "@/components/StateBadge";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import type { NetworkReading, NodeState, NodeStateWord } from "@/lib/network";

/**
 * Which of the four colours each state is drawn in — the same pairing the status strip uses.
 *
 * The strip and this card are two places one node is shown, and two mappings of state to colour
 * would be two answers to *is anything wrong*.
 */
const TONE: Record<NodeStateWord, StateTone> = {
  stopped: "idle",
  starting: "warn",
  running: "ok",
  failing: "bad",
};

/**
 * The identifiers this build knows how to read.
 *
 * A node's own list is allowed to grow without this being rebuilt, so anything from outside it is
 * drawn with the general sentence instead of being put on screen raw. The identifier is already in
 * the records — the Rust side writes it the moment it happens — so nothing is lost by keeping it
 * off the screen.
 */
const SAYS = {
  already_on_a_network: "alreadyOnANetwork",
  no_such_network: "noSuchNetwork",
  nobody_is_there: "nobodyIsThere",
  zone_silent: "zoneSilent",
  the_zone_is_unreadable: "theZoneIsUnreadable",
  seed_unreachable: "seedUnreachable",
  seed_would_not_answer: "seedWouldNotAnswer",
  seed_too_slow: "seedTooSlow",
  not_the_promised_network: "notThePromisedNetwork",
  there_is_a_network: "thereIsANetwork",
  no_randomness: "noRandomness",
  no_clock: "noClock",
  no_directory: "noDirectory",
  unreadable_identity: "unreadableIdentity",
  unreadable_record: "unreadableRecord",
  record_does_not_add_up: "recordDoesNotAddUp",
  record_would_not_start: "recordWouldNotStart",
  format_is_not_frozen: "formatIsNotFrozen",
  directory_held: "directoryHeld",
  directory_cannot_be_held: "directoryCannotBeHeld",
  no_transport: "noTransport",
  mesh_address_unavailable: "meshAddressUnavailable",
  address_unavailable: "addressUnavailable",
  no_network: "noNetwork",
  government_key_not_kept: "governmentKeyNotKept",
  resolver_not_an_address: "resolverNotAnAddress",
  not_erased: "notErased",
} as const;

/** The catalogue key for one refusal, or the general one where this build has never heard of it. */
export function reasonFor(code: string) {
  const known = Object.entries(SAYS).find(([said]) => said === code)?.[1];
  return known === undefined
    ? ("network.about.reason.unknown" as const)
    : (`network.about.reason.${known}` as const);
}

/** What the head is drawn from. */
interface NetworkFactsProps {
  /** The last reading, or `null` before the first one has come back. */
  reading: NetworkReading | null;
  /** What the node was doing at that same moment, or `null` before the first look. */
  state: NodeState | null;
}

/** The figures at the head of the Network screen. */
function NetworkFacts({ reading, state }: NetworkFactsProps) {
  const { t } = useTranslation();

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("network.about.heading")}</CardTitle>
      </CardHeader>

      <CardContent className="flex flex-col gap-4">
        {/* Nothing at all until the first look has come back: a card saying *stopped* over a node
            that is starting would be an assertion nobody made. */}
        {state !== null && (
          <div className="flex flex-wrap items-center gap-3">
            <StateBadge tone={TONE[state.state]} label={t(`status.node.${state.state}`)} />
            {state.failing !== null && (
              <span className="text-sm text-destructive">{t(reasonFor(state.failing))}</span>
            )}
          </div>
        )}

        {/* The figures flow the way cards do, and for the same reason: at 400 points across
            three of them do not fit side by side, and a row that overflowed would be the one
            place you could scroll sideways. The names span every column of it, however many
            the width turned out to be worth. */}
        <div className="grid grid-cols-[repeat(auto-fit,minmax(min(100%,var(--width-figure-min)),1fr))] gap-4">
          <Figure
            label={t("network.about.figure.network")}
            value={reading?.network ?? null}
            identifier
          />
          <Figure
            label={t("network.about.figure.identity")}
            value={reading?.identity ?? null}
            identifier
          />
          <Figure
            label={t("network.about.figure.root")}
            value={reading?.root ?? null}
            identifier
          />
          <Figure
            label={t("network.about.figure.peer")}
            value={reading?.peer ?? null}
            identifier
          />
          <Figure
            label={t("network.about.figure.written")}
            value={
              reading?.written === null || reading?.written === undefined
                ? null
                : String(reading.written)
            }
          />
          <Figure
            label={t("network.about.figure.peers")}
            value={
              reading?.peers === null || reading?.peers === undefined
                ? null
                : String(reading.peers)
            }
          />
          <Figure
            label={t("network.about.figure.silent")}
            value={
              reading?.silent === null || reading?.silent === undefined
                ? null
                : String(reading.silent)
            }
          />
        </div>
      </CardContent>
    </Card>
  );
}

export default NetworkFacts;
