/**
 * How much record traffic is crossing this node, over the last few minutes.
 *
 * # A rate is two totals and the time between them
 *
 * The node counts bytes and nothing else: totals since it came up, in and out. It keeps no history
 * and it is right not to — how often to sample and how long to remember are decisions about a
 * drawing, and a node that made them for everybody would be a node with a chart's opinions in it.
 *
 * So this samples. Every {@link EVERY_MS} it reads the two totals, takes the difference from the
 * sample before and divides by the time that actually passed — not by the interval, because a
 * timer that was late would otherwise read as a burst.
 *
 * # The history is this screen's and it starts when the screen does
 *
 * **Opening this screen starts the chart, and leaving it ends it.** Nothing is stored, so a window
 * that was put away for an hour comes back to an empty chart rather than to a flat line it did not
 * measure. It is the honest shape: a line is drawn only over the minutes somebody was looking, and
 * a gap is a gap.
 *
 * The first sample draws nothing. A rate needs two totals, and the first read has only one — a
 * chart that opened with a point at nought would be a measurement nobody took.
 *
 * # In is up and out is down
 *
 * The two directions are mirrored around nought, which is how every node application draws this and
 * is the one arrangement where *am I giving more than I take* is answered by the shape rather than
 * by reading two numbers. Out is stored negative and labelled positive: the axis is a magnitude
 * either way, and a tooltip saying `-4 KiB/s` would be describing a direction as a debt.
 */

import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Area, AreaChart, CartesianGrid, XAxis, YAxis } from "recharts";

import {
  ChartContainer,
  ChartTooltip,
  ChartTooltipContent,
  type ChartConfig,
} from "@/components/ui/chart";
import { readCrossed } from "@/lib/network";
import { inBytes } from "@/lib/sizes";

/**
 * How often the totals are read.
 *
 * Two seconds: fine enough that a page of acts arriving is a shape rather than a single spike, and
 * coarse enough to be nothing at all on a battery. It is not the ten seconds the rest of the
 * application looks on — a chart at that cadence is six points a minute, which is a list.
 */
const EVERY_MS = 2000;

/**
 * How many samples are kept, which is how far back the chart goes.
 *
 * Ninety at two seconds is three minutes. Long enough to see a sync happen and come back down,
 * short enough that the line is not a wall of pixels nobody can read.
 */
const KEPT = 90;

/** One moment: what was crossing, each way, in bytes per second. */
interface Sample {
  /** When it was taken, for the axis. */
  at: number;
  /** Bytes per second read off the wire. Drawn upwards. */
  taken: number;
  /** Bytes per second written to the wire, held negative so that it draws downwards. */
  given: number;
}


/** What is crossing this node, over the last few minutes. */
function Traffic() {
  const { t, i18n } = useTranslation();

  /* The two series, named where the chart's own tooltip reads them from. Built here rather than
     beside the file's constants because the names are translated, and a catalogue key is a literal
     to the type checker — one looked up through a value is a key nothing can hold this file to. */
  const series = {
    taken: { label: t("traffic.in"), color: "var(--traffic-in)" },
    given: { label: t("traffic.out"), color: "var(--traffic-out)" },
  } satisfies ChartConfig;

  const [samples, setSamples] = useState<Sample[]>([]);
  /* The totals from the previous read, and when. A ref rather than state: it is what the next
     sample is computed from and nothing draws it, so writing it must not cause a render. */
  const before = useRef<{ taken: number; given: number; at: number } | null>(null);
  /* Whether the node has a place on the mesh at all, which is a different emptiness from having
     one and nothing crossing it. `null` until the first read comes back. */
  const [onTheMesh, setOnTheMesh] = useState<boolean | null>(null);

  useEffect(() => {
    let alive = true;

    const look = async () => {
      const crossed = await readCrossed().catch(() => null);
      if (!alive) return;
      setOnTheMesh(crossed !== null);
      if (crossed === null) {
        // No mesh, so nothing is crossing and no history is worth keeping: coming back up starts
        // a new line rather than continuing one across a gap it did not measure.
        before.current = null;
        setSamples([]);
        return;
      }

      const now = Date.now();
      const last = before.current;
      before.current = { taken: crossed.taken, given: crossed.given, at: now };
      // The first read has a total and no rate. Nothing is drawn from it.
      if (last === null) return;

      const seconds = (now - last.at) / 1000;
      if (seconds <= 0) return;
      setSamples((had) =>
        [
          ...had,
          {
            at: now,
            // Divided by the time that actually passed, so a late timer is not a burst. Clamped
            // at nought because a counter cannot go backwards, and one that appeared to has been
            // restarted underneath us.
            taken: Math.max(0, (crossed.taken - last.taken) / seconds),
            given: -Math.max(0, (crossed.given - last.given) / seconds),
          },
        ].slice(-KEPT),
      );
    };

    void look();
    const timer = setInterval(() => {
      void look();
    }, EVERY_MS);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  }, []);

  /** A rate on the axis and in the tooltip, as a magnitude either way. */
  const perSecond = (bytes: number) => `${inBytes(Math.abs(bytes), i18n.language)}/s`;

  if (onTheMesh === false) {
    return (
      <p className="text-muted-foreground text-sm">{t("traffic.noMesh")}</p>
    );
  }

  return (
    <div className="flex flex-col gap-2">
      <p className="text-muted-foreground text-xs tracking-wide uppercase">
        {t("traffic.heading")}
      </p>

      {samples.length < 2 ? (
        /* In the document from the start rather than appearing: it is what the chart's own space
           says while there is not yet a rate to draw, and a box that popped into existence two
           seconds in would read as something having gone wrong. */
        <p className="text-muted-foreground flex h-[140px] items-center text-sm">
          {t("traffic.measuring")}
        </p>
      ) : (
        <ChartContainer config={series} className="h-[140px] w-full">
          <AreaChart data={samples} margin={{ left: 4, right: 4, top: 4, bottom: 4 }}>
            <CartesianGrid vertical={false} strokeDasharray="3 3" />
            <XAxis
              dataKey="at"
              tickLine={false}
              axisLine={false}
              tickMargin={8}
              minTickGap={48}
              tickFormatter={(at: number) =>
                new Date(at).toLocaleTimeString(i18n.language, {
                  hour: "2-digit",
                  minute: "2-digit",
                })
              }
            />
            <YAxis
              tickLine={false}
              axisLine={false}
              width={64}
              tickFormatter={perSecond}
            />
            <ChartTooltip
              content={
                <ChartTooltipContent
                  labelFormatter={(_, payload) =>
                    new Date(payload[0]?.payload.at ?? Date.now()).toLocaleTimeString(
                      i18n.language,
                    )
                  }
                  formatter={(value, name) => (
                    <span className="flex w-full justify-between gap-4">
                      <span>{series[name as keyof typeof series].label}</span>
                      <span className="font-mono">{perSecond(Number(value))}</span>
                    </span>
                  )}
                />
              }
            />
            <Area
              dataKey="taken"
              type="monotone"
              stroke="var(--color-taken)"
              fill="var(--color-taken)"
              fillOpacity={0.25}
              isAnimationActive={false}
            />
            <Area
              dataKey="given"
              type="monotone"
              stroke="var(--color-given)"
              fill="var(--color-given)"
              fillOpacity={0.25}
              isAnimationActive={false}
            />
          </AreaChart>
        </ChartContainer>
      )}
    </div>
  );
}

export default Traffic;
