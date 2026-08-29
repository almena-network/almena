/**
 * The two things an operator can do to a node from here.
 *
 * **The same two the terminal offers**, because neither way of running a node may be able to do
 * something the other cannot. What each one means, and every rule about when it is allowed, lives
 * below the interface; this draws buttons and shows what came back.
 *
 * Opening a network is offered only while there is none. It is not a thing to do twice: a node is
 * a directory with a key in it, and a second network over the same directory would be a second
 * history for one identity.
 *
 * **What comes back when something fails is an identifier, never a sentence.** The node has no idea
 * what language anybody reads in, and two operators comparing notes need the same word — so the
 * word travels and the reading happens here.
 */

import { useState } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";

import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";

/** What the controls are drawn from. */
interface NodeControlsProps {
  /** Whether this node is on a network, which decides what can be done to it. */
  onNetwork: boolean;
  /** Called once something has changed, so the figures above are read again. */
  onChanged: () => void;
}

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
  no_randomness: "noRandomness",
  no_clock: "noClock",
  no_directory: "noDirectory",
  unreadable_identity: "unreadableIdentity",
  record_would_not_start: "recordWouldNotStart",
  unreadable_record: "unreadableRecord",
  record_does_not_add_up: "recordDoesNotAddUp",
  directory_held: "directoryHeld",
  directory_cannot_be_held: "directoryCannotBeHeld",
  no_certificate: "noCertificate",
  no_private_key: "noPrivateKey",
  certificate_and_key_are_not_a_pair: "certificateAndKeyAreNotAPair",
  zone_silent: "zoneSilent",
  there_is_a_network: "thereIsANetwork",
  not_the_promised_network: "notThePromisedNetwork",
  no_transport: "noTransport",
  mesh_address_unavailable: "meshAddressUnavailable",
  no_network: "noNetwork",
  address_unavailable: "addressUnavailable",
  not_a_claim: "notAClaim",
  not_theirs: "notTheirs",
  not_written_down: "notWrittenDown",
} as const;

/** The catalogue key for one refusal, or the general one where this build has never heard of it. */
function reasonFor(code: string) {
  const known = Object.entries(SAYS).find(([said]) => said === code)?.[1];
  return known === undefined
    ? ("network.control.reason.unknown" as const)
    : (`network.control.reason.${known}` as const);
}

/** Opening a network, and serving the interface. */
function NodeControls({ onNetwork, onChanged }: NodeControlsProps) {
  const { t } = useTranslation();
  // **This is a different node from the one in a terminal**, not another view of it: the two keep
  // separate data directories, so they hold separate keys, so they are two participants that
  // happen to share a computer. Two nodes cannot share a port, and somebody running both while
  // working should not have to find that out from a bind that failed.
  const [address, setAddress] = useState("127.0.0.1:8791");
  const [zone, setZone] = useState("");
  const [mesh, setMesh] = useState("4002");
  const [carry, setCarry] = useState(false);
  const [carriedBy, setCarriedBy] = useState("");
  const [certificate, setCertificate] = useState("");
  const [privateKey, setPrivateKey] = useState("");
  const [challenge, setChallenge] = useState("");
  const [approval, setApproval] = useState("");
  const [failed, setFailed] = useState<string | null>(null);

  /** Run one command, saying whether it worked and keeping whatever identifier came back. */
  async function run(
    command: string,
    argument?: Record<string, string | number | boolean | string[] | undefined>,
  ) {
    setFailed(null);
    try {
      await invoke(command, argument);
      onChanged();
      return true;
    } catch (reason) {
      // An identifier the node chose, or — if something unforeseen surfaced — nothing this can
      // claim to explain, which is drawn as the unrecognised case rather than as raw text.
      setFailed(typeof reason === "string" ? reason : "");
      return false;
    }
  }

  /** Ask the node for a challenge, and keep it on screen until it is used or replaced. */
  async function showChallenge() {
    setFailed(null);
    try {
      // A day, in epochs. Long enough to walk to another machine and short enough that one left
      // in a screenshot does not bind anybody's machine a year later.
      const shown = await invoke<string>("who_contributed_me", {
        forEpochs: 24,
      });
      setChallenge(shown);
    } catch (reason) {
      setFailed(typeof reason === "string" ? reason : "");
    }
  }

  return (
    <Card>
      <CardHeader>
        <CardTitle>{t("network.control.heading")}</CardTitle>
        <CardDescription>{t("network.control.body")}</CardDescription>
      </CardHeader>

      <CardContent className="flex flex-col gap-4">
        {!onNetwork && (
          <div className="flex gap-2">
            {/* Empty means the network's own zone. Pointing this somewhere else is for an operator
                running a network of their own — the check it feeds, open only when nobody is
                there, is worth nothing if the zone it asked about was not the network's. */}
            <input
              className="border-input bg-transparent h-9 flex-1 rounded-md border px-3 text-sm"
              aria-label={t("network.control.zone")}
              placeholder={t("network.control.zone")}
              value={zone}
              onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
                setZone(event.target.value)
              }
            />
            <Button
              onClick={() =>
                void run("open_development_network", {
                  zone: zone.trim() || undefined,
                })
              }
            >
              {t("network.control.open")}
            </Button>
          </div>
        )}

        {/* The node closes its own epochs on a timer from the moment it is on a network. This is
            for not waiting for it, which is why it is offered only once there is something to
            close. */}
        <Button
          variant="secondary"
          disabled={!onNetwork}
          onClick={() => void run("close_epoch")}
        >
          {t("network.control.closeEpoch")}
        </Button>

        <div className="flex gap-2">
          {/* A plain field rather than a vendored one: this is the only text anybody types into
              the whole application, and adding an element to the registry to hold it would be a
              dependency taken on for one input. */}
          <input
            className="border-input bg-transparent h-9 flex-1 rounded-md border px-3 text-sm"
            aria-label={t("network.control.address")}
            value={address}
            onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
              setAddress(event.target.value)
            }
          />
          <Button
            disabled={!onNetwork}
            onClick={() =>
              void run("serve_interface", {
                address,
                // Empty means none. A node with no certificate answers in the clear, which is
                // right on the machine it runs on and wrong anywhere else — so it is a thing to
                // ask for rather than a thing to be given quietly.
                certificate: certificate.trim() || undefined,
                privateKey: privateKey.trim() || undefined,
              })
            }
          >
            {t("network.control.serve")}
          </Button>
        </div>

        <div className="flex gap-2">
          {/* Chosen and not discovered: it is the port somebody publishes in the zone, and a node
              that took whatever was free would make that record wrong on its next start. */}
          <input
            className="border-input bg-transparent h-9 flex-1 rounded-md border px-3 text-sm"
            aria-label={t("network.control.mesh")}
            placeholder={t("network.control.mesh")}
            value={mesh}
            onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
              setMesh(event.target.value)
            }
          />
          <Button
            disabled={!onNetwork}
            onClick={() =>
              void run("join_the_mesh", {
                port: Number(mesh) || 0,
                carry,
                // Empty means nobody to ask, which is right for a node that can be dialled.
                carriedBy: carriedBy.trim() ? [carriedBy.trim()] : [],
              })
            }
          >
            {t("network.control.joinTheMesh")}
          </Button>
        </div>

        <div className="flex items-center gap-2">
          {/* Volunteered, never assumed: it spends this machine's bandwidth on somebody else's
              conversation, and turning it on says so in the record where it is counted. */}
          <input
            id="carry"
            type="checkbox"
            checked={carry}
            onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
              setCarry(event.target.checked)
            }
          />
          <label htmlFor="carry" className="text-sm">
            {t("network.control.carry")}
          </label>
        </div>

        {/* For a node that cannot be dialled: behind a household router there is no door anybody
            outside can knock on, and somebody carrying it is what turns it back into a node. The
            address has to name the relay — being carried by whoever answers at a host and port is
            being carried by whoever took them. */}
        <input
          className="border-input bg-transparent h-9 rounded-md border px-3 text-sm"
          aria-label={t("network.control.carriedBy")}
          placeholder={t("network.control.carriedBy")}
          value={carriedBy}
          onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
            setCarriedBy(event.target.value)
          }
        />

        <div className="flex gap-2">
          <input
            className="border-input bg-transparent h-9 flex-1 rounded-md border px-3 text-sm"
            aria-label={t("network.control.certificate")}
            placeholder={t("network.control.certificate")}
            value={certificate}
            onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
              setCertificate(event.target.value)
            }
          />
          <input
            className="border-input bg-transparent h-9 flex-1 rounded-md border px-3 text-sm"
            aria-label={t("network.control.privateKey")}
            placeholder={t("network.control.privateKey")}
            value={privateKey}
            onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
              setPrivateKey(event.target.value)
            }
          />
        </div>

        {/* Whoever sustains the network earns the right to write on it, and that has to attach to
            somebody: a node nobody claimed is a machine, and a machine cannot be credited. The node
            can only ask — approving is done by whoever contributed it, with their own key, wherever
            that key lives. */}
        <div className="flex flex-col gap-2 border-t pt-4">
          <p className="text-sm font-medium">
            {t("network.control.claimHeading")}
          </p>
          <p className="text-muted-foreground text-sm">
            {t("network.control.claimBody")}
          </p>
          <Button
            variant="secondary"
            disabled={!onNetwork}
            onClick={() => void showChallenge()}
          >
            {t("network.control.showChallenge")}
          </Button>
          {challenge !== "" && (
            <>
              <p className="text-muted-foreground text-sm">
                {t("network.control.challengeShown")}
              </p>
              {/* Selectable text and never an input: it is the node's to show, not anybody's to
                  edit, and what gets approved has to be what was shown. */}
              <p className="bg-muted rounded-md p-2 font-mono text-xs break-all select-all">
                {challenge}
              </p>
              <input
                className="border-input bg-transparent h-9 rounded-md border px-3 font-mono text-sm"
                aria-label={t("network.control.approval")}
                placeholder={t("network.control.approval")}
                value={approval}
                onChange={(event: React.ChangeEvent<HTMLInputElement>) =>
                  setApproval(event.target.value)
                }
              />
              <Button
                disabled={approval.trim() === ""}
                onClick={() =>
                  void run("contributed_by", {
                    challenge,
                    approval: approval.trim(),
                  }).then((written) => {
                    // Shown once and gone, like the challenge itself: what is worth reading now
                    // lives in the record, where anybody can. A refusal keeps both on screen,
                    // because the fix is usually pasting again.
                    if (written) {
                      setChallenge("");
                      setApproval("");
                    }
                  })
                }
              >
                {t("network.control.recordClaim")}
              </Button>
            </>
          )}
          <Button
            variant="secondary"
            disabled={!onNetwork}
            onClick={() => void run("contributed_by_nobody")}
          >
            {t("network.control.letGo")}
          </Button>
        </div>

        {failed !== null && (
          <p className="text-sm text-destructive">
            {t(reasonFor(failed))}
          </p>
        )}
      </CardContent>
    </Card>
  );
}

export default NodeControls;
