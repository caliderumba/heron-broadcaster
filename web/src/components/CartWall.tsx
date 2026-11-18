// SPDX-License-Identifier: AGPL-3.0-or-later

// Cart wall (M18): pages of pads that fire the FX player. `F1`–`F12` fire the
// current page, `Shift+F1`–`F12` choose a page and `PageUp`/`PageDown` move
// between them; a MIDI controller can learn a note or a control change per
// pad, kept in this browser.

import { useQuery } from "@tanstack/react-query";
import { type DragEvent, type FormEvent, useEffect, useRef, useState } from "react";
import {
  type Cart,
  type CartPage,
  type DeckState,
  api,
  done,
  unwrap,
} from "../api";
import { t } from "../i18n";
import { COLORS } from "../lib/colors";
import { duration } from "../lib/format";
import { positionNow, useLive } from "../lib/live";
import { here, currentStation } from "../lib/station";
import { useAction, useCan } from "../lib/session";
import { Card, ConfirmButton, Empty, ErrorView, Field, Icon, Loading, Modal, useToast } from "./ui";
import { TrackSearch } from "./TrackSearch";

/** Pads per page, one per function key. */
const PADS = 12;
const MIDI_KEY = "heron.carts.midi";

/** Re-renders `fps` times a second while mounted. */
function useTick(fps = 4): number {
  const [now, setNow] = useState(() => performance.now());
  useEffect(() => {
    let frame = 0;
    let last = 0;
    const step = (time: number) => {
      if (time - last >= 1000 / fps) {
        last = time;
        setNow(time);
      }
      frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, [fps]);
  return now;
}

/**
 * The control a MIDI message stands for (`note:60:1`, `cc:7:1`, channel
 * last), or `null` when it is not a note-on or a control change.
 */
export function midiBinding(data: Uint8Array | readonly number[]): string | null {
  const [status = 0, number = 0, value = 0] = Array.from(data);
  const kind = status & 0xf0;
  const channel = status & 0x0f;
  if (kind === 0x90 && value > 0) return `note:${number}:${channel}`;
  if (kind === 0xb0) return `cc:${number}:${channel}`;
  return null;
}

/** What a binding says, for people: `Note 60 · ch 1`. */
export function midiLabel(binding: string): string {
  const [kind, number = "?", channel = ""] = binding.split(":");
  const name = kind === "cc" ? "CC" : "Note";
  return `${name} ${number} · ch ${Number(channel) + 1}`;
}

function storedMidi(): Record<string, string> {
  try {
    const raw = localStorage.getItem(MIDI_KEY);
    const parsed = raw ? (JSON.parse(raw) as unknown) : null;
    return typeof parsed === "object" && parsed !== null ? (parsed as Record<string, string>) : {};
  } catch {
    return {};
  }
}

const isTyping = (target: EventTarget | null): boolean =>
  target instanceof HTMLElement &&
  (target.isContentEditable || target.closest("input, textarea, select, [contenteditable='true']") !== null);

export function CartWall({ fx, paused }: Readonly<{ fx: DeckState | undefined; paused: boolean }>) {
  const canEdit = useCan("operator");
  const toast = useToast();
  const [open, setOpen] = useState(true);
  const [editing, setEditing] = useState(false);
  const [selected, setSelected] = useState<number | null>(null);
  const [dialog, setDialog] = useState<
    | { kind: "page"; page: CartPage | null }
    | { kind: "cart"; page: CartPage; cart: Cart | null; position: number }
    | { kind: "midi"; cart: Cart }
    | null
  >(null);
  const [midi, setMidi] = useState<Record<string, string>>(storedMidi);
  const [learning, setLearning] = useState<number | null>(null);
  const panel = useRef<HTMLDivElement>(null);
  const learningRef = useRef<number | null>(null);
  const cartsRef = useRef<Cart[]>([]);
  const midiRef = useRef(midi);
  const fireRef = useRef<(cart: Cart) => void>(() => {});

  const pages = useQuery({
    queryKey: ["cart-pages", currentStation()],
    queryFn: () =>
      unwrap(api.GET("/api/v1/stations/{station_id}/cart-pages", { params: { path: here() } })),
    enabled: canEdit,
  });

  const stop = useAction(
    () => done(api.DELETE("/api/v1/stations/{station_id}/player/fx", { params: { path: here() } })),
    { success: t("carts.stopped") },
  );
  const fire = useAction(
    (cart: Cart) =>
      unwrap(
        api.POST("/api/v1/stations/{station_id}/carts/{id}/play", {
          params: { path: here({ id: cart.id }) },
        }),
      ),
    {},
  );
  const move = useAction(
    ({ cart, position }: { cart: Cart; position: number }) =>
      unwrap(
        api.PUT("/api/v1/stations/{station_id}/carts/{id}", {
          params: { path: here({ id: cart.id }) },
          body: {
            page_id: cart.page_id,
            track_id: cart.track_id,
            label: cart.label,
            color: cart.color ?? null,
            position,
          },
        }),
      ),
    { invalidate: [["cart-pages"]] },
  );
  const removePage = useAction(
    (id: number) =>
      done(api.DELETE("/api/v1/stations/{station_id}/cart-pages/{id}", { params: { path: here({ id }) } })),
    {
      success: t("carts.pageDeleted"),
      invalidate: [["cart-pages"]],
      onSuccess: () => setSelected(null),
    },
  );

  const list = pages.data ?? [];
  const current = list.find((page) => page.id === selected) ?? list[0];
  const carts = current?.carts ?? [];
  const positions = useLive((live) => live.positions);
  const tick = useTick();
  const fxPosition = fx
    ? (positionNow(positions, "fx", paused, tick)?.position_ms ?? fx.position_ms)
    : 0;
  const onAir = (cart: Cart) => fx?.entry?.track.track_id === cart.track_id;

  useEffect(() => {
    learningRef.current = learning;
  }, [learning]);
  useEffect(() => {
    midiRef.current = midi;
    try {
      localStorage.setItem(MIDI_KEY, JSON.stringify(midi));
    } catch {
      // Not remembered in this browser.
    }
  }, [midi]);
  cartsRef.current = carts;
  fireRef.current = (cart) => fire.mutate(cart);

  // MIDI: a note or control change fires the pad it is mapped to, or is
  // learned while a pad's dialog waits for one.
  useEffect(() => {
    const request = (navigator as { requestMIDIAccess?: () => Promise<MIDIAccess> }).requestMIDIAccess;
    if (typeof request !== "function") return;
    let cancelled = false;
    let inputs: MIDIInput[] = [];
    const message = (event: MIDIMessageEvent) => {
      if (!event.data) return;
      const binding = midiBinding(event.data);
      if (!binding) return;
      const id = learningRef.current;
      if (id !== null) {
        setMidi((map) => ({ ...map, [String(id)]: binding }));
        learningRef.current = null;
        setLearning(null);
        toast(t("carts.midiLearned", { binding: midiLabel(binding) }));
        return;
      }
      const cart = cartsRef.current.find((item) => midiRef.current[String(item.id)] === binding);
      if (cart) fireRef.current(cart);
    };
    request
      .call(navigator)
      .then((access) => {
        if (cancelled) return;
        const attach = () => {
          for (const input of inputs) input.removeEventListener("midimessage", message);
          inputs = [...access.inputs.values()];
          for (const input of inputs) input.addEventListener("midimessage", message);
        };
        attach();
        access.onstatechange = attach;
      })
      .catch(() => {
        // No MIDI in this browser: the keys still work.
      });
    return () => {
      cancelled = true;
      for (const input of inputs) input.removeEventListener("midimessage", message);
    };
  }, [toast]);

  // Keys: fire a pad, change page, or stop the FX while the panel has focus.
  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (isTyping(event.target)) return;
      const function_ = /^F([1-9]|1[0-2])$/.exec(event.key);
      if (function_) {
        event.preventDefault();
        if (event.repeat) return;
        const index = Number(function_[1]) - 1;
        if (event.shiftKey) {
          const page = list[index];
          if (page) setSelected(page.id);
        } else {
          const cart = carts[index];
          if (cart && !cart.missing) fire.mutate(cart);
        }
        return;
      }
      if (event.key === "PageUp" || event.key === "PageDown") {
        event.preventDefault();
        if (event.repeat || list.length === 0) return;
        const at = list.findIndex((page) => page.id === current?.id);
        const next = event.key === "PageUp" ? at - 1 : at + 1;
        const page = list[(next + list.length) % list.length];
        if (page) setSelected(page.id);
        return;
      }
      if (
        event.key === "Escape" &&
        fx?.entry &&
        panel.current?.contains(document.activeElement)
      ) {
        event.preventDefault();
        stop.mutate(undefined);
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  }, [carts, current?.id, fire, fx?.entry, list, stop]);

  if (!canEdit) {
    return null;
  }

  const openAdd = (position: number) => {
    if (current) setDialog({ kind: "cart", page: current, cart: null, position });
  };

  return (
    <div ref={panel} className="cart-wall">
      <Card
        title={t("carts.title")}
        actions={
          <>
            {fx?.entry && (
              <button type="button" className="btn small" onClick={() => stop.mutate(undefined)}>
                <Icon name="stop" size={14} /> {t("dashboard.stop")}
              </button>
            )}
            <button type="button" className="btn small" aria-expanded={open} onClick={() => setOpen(!open)}>
              {open ? t("carts.hide") : t("carts.show")}
            </button>
            <button
              type="button"
              className="btn small"
              aria-pressed={editing}
              onClick={() => setEditing(!editing)}
            >
              <Icon name="edit" size={14} /> {editing ? t("carts.done") : t("carts.edit")}
            </button>
          </>
        }
      >
        {open && (
          <div className="stack">
            {pages.isPending && <Loading />}
            {pages.isError && <ErrorView error={pages.error} />}
            {list.length === 0 && pages.data && (
              <Empty>
                <span className="row" style={{ justifyContent: "center" }}>
                  {t("carts.noPages")}{" "}
                  <button type="button" className="btn small" onClick={() => setDialog({ kind: "page", page: null })}>
                    <Icon name="plus" size={14} /> {t("carts.newPage")}
                  </button>
                </span>
              </Empty>
            )}
            {current && (
              <>
                <div className="tabs" role="tablist" aria-label={t("carts.title")}>
                  {list.map((page) => (
                    <button
                      key={page.id}
                      type="button"
                      role="tab"
                      aria-selected={page.id === current.id}
                      onClick={() => setSelected(page.id)}
                    >
                      {page.name}
                      <span className="count">{page.carts.length}</span>
                    </button>
                  ))}
                  {editing && (
                    <button type="button" onClick={() => setDialog({ kind: "page", page: null })}>
                      <Icon name="plus" size={14} /> {t("carts.newPage")}
                    </button>
                  )}
                </div>
                {editing && (
                  <div className="row cart-edit-bar">
                    <span className="muted small">{t("carts.editingHint")}</span>
                    <button
                      type="button"
                      className="btn small"
                      onClick={() => setDialog({ kind: "page", page: current })}
                    >
                      <Icon name="edit" size={14} /> {t("carts.editPage")}
                    </button>
                    <ConfirmButton
                      className="btn ghost small danger"
                      confirm={t("carts.deletePageButton")}
                      question={t("carts.deletePageQuestion", { name: current.name, count: current.carts.length })}
                      onConfirm={() => removePage.mutate(current.id)}
                    >
                      <Icon name="trash" size={14} /> {t("carts.deletePage")}
                    </ConfirmButton>
                  </div>
                )}
                <div className="cart-pads" role="group" aria-label={t("carts.pads", { page: current.name })}>
                  {Array.from({ length: PADS }, (_, index) => {
                    const cart = carts[index];
                    if (!cart) {
                      return editing ? (
                        <button
                          key={`empty-${index}`}
                          type="button"
                          className="cart-pad empty"
                          aria-label={t("carts.addToSlot", { key: `F${index + 1}` })}
                          onClick={() => openAdd(index)}
                        >
                          <Icon name="plus" />
                          <span className="cart-key">{`F${index + 1}`}</span>
                        </button>
                      ) : (
                        <span key={`empty-${index}`} className="cart-pad empty" aria-hidden="true">
                          <span className="cart-key">{`F${index + 1}`}</span>
                        </span>
                      );
                    }
                    const progress =
                      cart.duration_ms && onAir(cart)
                        ? Math.min(1, fxPosition / cart.duration_ms)
                        : 0;
                    return (
                      <button
                        key={cart.id}
                        type="button"
                        className={`cart-pad${onAir(cart) ? " on-air" : ""}${cart.color ? " colored" : ""}${cart.missing ? " missing" : ""}`}
                        style={cart.color ? { background: cart.color, borderColor: cart.color } : undefined}
                        aria-pressed={onAir(cart)}
                        title={cart.missing ? t("carts.missingHint") : cart.label}
                        draggable={editing}
                        onClick={() =>
                          editing
                            ? setDialog({ kind: "cart", page: current, cart, position: index })
                            : cart.missing
                              ? toast(t("carts.missingHint"), true)
                              : fire.mutate(cart)
                        }
                        onDragStart={(event: DragEvent<HTMLButtonElement>) =>
                          event.dataTransfer.setData("text/plain", String(cart.id))
                        }
                        onDragOver={(event) => editing && event.preventDefault()}
                        onDrop={(event) => {
                          if (!editing) return;
                          event.preventDefault();
                          const source = carts.find(
                            (item) => item.id === Number(event.dataTransfer.getData("text/plain")),
                          );
                          if (source && source.id !== cart.id) {
                            move.mutate({ cart: source, position: index });
                          }
                        }}
                      >
                        <span className="cart-key">{`F${index + 1}`}</span>
                        <span className="cart-label ellipsis">{cart.label}</span>
                        <span className="cart-meta">
                          {cart.missing
                            ? t("carts.missing")
                            : cart.duration_ms
                              ? duration(cart.duration_ms)
                              : ""}
                        </span>
                        {midi[String(cart.id)] && (
                          <span className="cart-midi" title={midiLabel(midi[String(cart.id)] ?? "")}>
                            <Icon name="bolt" size={12} />
                          </span>
                        )}
                        {progress > 0 && (
                          <span className="cart-pad-progress" aria-hidden="true">
                            <span style={{ width: `${progress * 100}%` }} />
                          </span>
                        )}
                      </button>
                    );
                  })}
                </div>
                <p className="muted small" style={{ margin: 0 }}>
                  {t("carts.keyboard")}
                </p>
              </>
            )}
          </div>
        )}
      </Card>
      {dialog?.kind === "page" && (
        <PageDialog page={dialog.page} pages={list} onClose={() => setDialog(null)} />
      )}
      {dialog?.kind === "cart" && (
        <CartDialog
          cart={dialog.cart}
          page={dialog.page}
          position={dialog.position}
          pages={list}
          midiBinding={dialog.cart ? midi[String(dialog.cart.id)] : undefined}
          onLearn={() => dialog.cart && setDialog({ kind: "midi", cart: dialog.cart })}
          onClose={() => setDialog(null)}
        />
      )}
      {dialog?.kind === "midi" && (
        <MidiDialog
          cart={dialog.cart}
          binding={midi[String(dialog.cart.id)]}
          learning={learning === dialog.cart.id}
          onLearn={() => setLearning(dialog.cart.id)}
          onClear={() => {
            setMidi((map) => {
              const next = { ...map };
              delete next[String(dialog.cart.id)];
              return next;
            });
            toast(t("carts.midiCleared"));
          }}
          onClose={() => {
            setLearning(null);
            setDialog(null);
          }}
        />
      )}
    </div>
  );
}

function PageDialog({
  page,
  pages,
  onClose,
}: Readonly<{ page: CartPage | null; pages: CartPage[]; onClose: () => void }>) {
  const [name, setName] = useState(page?.name ?? "");
  const [position, setPosition] = useState(page?.position ?? pages.length);
  const save = useAction(
    () =>
      page
        ? unwrap(
            api.PUT("/api/v1/stations/{station_id}/cart-pages/{id}", {
              params: { path: here({ id: page.id }) },
              body: { name, position },
            }),
          )
        : unwrap(
            api.POST("/api/v1/stations/{station_id}/cart-pages", {
              params: { path: here() },
              body: { name, position },
            }),
          ),
    { success: t("carts.pageSaved"), invalidate: [["cart-pages"]], onSuccess: onClose },
  );
  const submit = (event: FormEvent) => {
    event.preventDefault();
    save.mutate(undefined);
  };
  return (
    <Modal
      title={page ? t("carts.editPageTitle", { name: page.name }) : t("carts.newPage")}
      open
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn" onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button type="submit" form="cart-page-form" className="btn primary" disabled={save.isPending || !name.trim()}>
            {t("common.save")}
          </button>
        </>
      }
    >
      <form id="cart-page-form" className="stack" onSubmit={submit}>
        <Field label={t("carts.pageName")}>
          <input
            className="input"
            value={name}
            onChange={(event) => setName(event.target.value)}
            required
            maxLength={64}
            autoFocus
          />
        </Field>
        <Field label={t("carts.position")} hint={t("carts.positionHint")}>
          <select className="input" value={position} onChange={(event) => setPosition(Number(event.target.value))}>
            {Array.from({ length: pages.length + (page ? 0 : 1) }, (_, index) => (
              <option key={index} value={index}>
                {index + 1}
              </option>
            ))}
          </select>
        </Field>
      </form>
    </Modal>
  );
}

function CartDialog({
  cart,
  page,
  position,
  pages,
  midiBinding,
  onLearn,
  onClose,
}: Readonly<{
  cart: Cart | null;
  page: CartPage;
  position: number;
  pages: CartPage[];
  midiBinding: string | undefined;
  onLearn: () => void;
  onClose: () => void;
}>) {
  const [track, setTrack] = useState<{ id: number; title: string } | null>(
    cart ? { id: cart.track_id, title: cart.title ?? cart.label } : null,
  );
  const [label, setLabel] = useState(cart?.label ?? "");
  const [color, setColor] = useState<string | null>(cart?.color ?? null);
  const [pageId, setPageId] = useState(page.id);
  const save = useAction(
    () => {
      const body = {
        page_id: pageId,
        track_id: track?.id ?? 0,
        label: label.trim() || null,
        color,
      };
      return cart
        ? unwrap(
            api.PUT("/api/v1/stations/{station_id}/carts/{id}", {
              params: { path: here({ id: cart.id }) },
              body,
            }),
          )
        : unwrap(
            api.POST("/api/v1/stations/{station_id}/carts", {
              params: { path: here() },
              body: { ...body, position },
            }),
          );
    },
    { success: t("carts.saved"), invalidate: [["cart-pages"]], onSuccess: onClose },
  );
  const remove = useAction(
    (id: number) => done(api.DELETE("/api/v1/stations/{station_id}/carts/{id}", { params: { path: here({ id }) } })),
    { success: t("carts.deleted"), invalidate: [["cart-pages"]], onSuccess: onClose },
  );
  const submit = (event: FormEvent) => {
    event.preventDefault();
    if (track) save.mutate(undefined);
  };
  return (
    <Modal
      title={cart ? t("carts.editTitle", { name: cart.label }) : t("carts.addTitle")}
      open
      onClose={onClose}
      wide
      footer={
        <>
          {cart && (
            <ConfirmButton
              className="btn ghost danger"
              confirm={t("carts.deleteButton")}
              question={t("carts.deleteQuestion", { name: cart.label })}
              onConfirm={() => remove.mutate(cart.id)}
            >
              <Icon name="trash" size={14} /> {t("carts.delete")}
            </ConfirmButton>
          )}
          <button type="button" className="btn" onClick={onClose}>
            {t("common.cancel")}
          </button>
          <button type="submit" form="cart-form" className="btn primary" disabled={!track || save.isPending}>
            {t("common.save")}
          </button>
        </>
      }
    >
      <form id="cart-form" className="stack" onSubmit={submit}>
        {!cart && (
          <>
            <p className="muted small" style={{ margin: 0 }}>
              {t("carts.chooseTrack")}
            </p>
            <TrackSearch
              renderActions={(result) => (
                <button
                  type="button"
                  className={track?.id === result.id ? "btn small primary" : "btn small"}
                  aria-pressed={track?.id === result.id}
                  onClick={() => {
                    setTrack({ id: result.id, title: result.title ?? result.path });
                    if (!label.trim()) setLabel(result.title ?? "");
                  }}
                >
                  {t("carts.choose")}
                </button>
              )}
            />
          </>
        )}
        {track && (
          <p className="small">
            <strong>{track.title}</strong>
          </p>
        )}
        <Field label={t("carts.label")} hint={t("carts.labelHint")}>
          <input
            className="input"
            value={label}
            onChange={(event) => setLabel(event.target.value)}
            maxLength={64}
          />
        </Field>
        <Field label={t("carts.color")}>
          <div className="row">
            <button
              type="button"
              className="btn small"
              aria-pressed={color === null}
              onClick={() => setColor(null)}
            >
              {t("carts.defaultColor")}
            </button>
            {COLORS.map((swatch) => (
              <button
                key={swatch}
                type="button"
                className="btn icon small"
                aria-label={t("categories.colorValue", { color: swatch })}
                aria-pressed={color === swatch}
                style={{
                  background: swatch,
                  borderColor: color === swatch ? "var(--fg)" : swatch,
                }}
                onClick={() => setColor(swatch)}
              />
            ))}
            <input
              type="color"
              value={color ?? COLORS[0]}
              onChange={(event) => setColor(event.target.value)}
              aria-label={t("categories.otherColor")}
            />
          </div>
        </Field>
        <Field label={t("carts.page")}>
          <select className="input" value={pageId} onChange={(event) => setPageId(Number(event.target.value))}>
            {pages.map((item) => (
              <option key={item.id} value={item.id}>
                {item.name}
              </option>
            ))}
          </select>
        </Field>
        {cart && (
          <Field label={t("carts.midi")} hint={t("carts.midiHelp")}>
            <span className="row">
              <button type="button" className="btn small" onClick={onLearn}>
                <Icon name="bolt" size={14} /> {t("carts.midiLearn")}
              </button>
              <span className="muted small">
                {midiBinding ? midiLabel(midiBinding) : t("carts.midiNone")}
              </span>
            </span>
          </Field>
        )}
      </form>
    </Modal>
  );
}

function MidiDialog({
  cart,
  binding,
  learning,
  onLearn,
  onClear,
  onClose,
}: Readonly<{
  cart: Cart;
  binding: string | undefined;
  learning: boolean;
  onLearn: () => void;
  onClear: () => void;
  onClose: () => void;
}>) {
  return (
    <Modal
      title={t("carts.midiTitle", { name: cart.label })}
      open
      onClose={onClose}
      footer={
        <>
          <button type="button" className="btn" onClick={onClose}>
            {t("ui.close")}
          </button>
          <button type="button" className="btn primary" onClick={onLearn}>
            {learning ? t("carts.midiWaiting") : t("carts.midiLearn")}
          </button>
        </>
      }
    >
      <div className="stack">
        <p style={{ margin: 0 }}>
          {binding ? t("carts.midiLearned", { binding: midiLabel(binding) }) : t("carts.midiNone")}
        </p>
        {learning && (
          <p className="muted" style={{ margin: 0 }} role="status">
            {t("carts.midiWaiting")}
          </p>
        )}
        <p className="muted small" style={{ margin: 0 }}>
          {t("carts.midiHelp")}
        </p>
        {binding && (
          <button type="button" className="btn small" onClick={onClear}>
            {t("carts.midiClear")}
          </button>
        )}
      </div>
    </Modal>
  );
}
