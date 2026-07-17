"use client";

import {
  ChangeEvent,
  FormEvent,
  KeyboardEvent,
  useCallback,
  useEffect,
  useRef,
  useState,
} from "react";

import { formatAppError, parseAppError } from "@/lib/app-error";
import {
  createWatchlist,
  deleteWatchlist,
  getWatchlist,
  listWatchlists,
  updateWatchlist,
} from "./api";
import {
  addSymbols,
  MAX_WATCHLIST_SYMBOLS,
  removeSymbol,
  restoreSymbol,
} from "./model";
import styles from "./WatchlistWorkspace.module.css";
import {
  emptyWatchlistForm,
  type WatchlistDetail,
  type WatchlistFormState,
  type WatchlistSummary,
} from "./types";

type PersistedMeta = {
  name: string;
  description: string | null;
};

type UndoRemoval = {
  symbol: string;
  index: number;
};

function detailToForm(detail: WatchlistDetail): WatchlistFormState {
  return {
    id: detail.id,
    name: detail.name,
    description: detail.description ?? "",
    symbols: [...detail.symbols],
  };
}

function detailToMeta(detail: WatchlistDetail): PersistedMeta {
  return {
    name: detail.name,
    description: detail.description,
  };
}

function symbolsMatch(left: string[], right: string[]): boolean {
  return left.length === right.length && left.every((symbol, index) => symbol === right[index]);
}

export default function WatchlistWorkspace() {
  const [watchlists, setWatchlists] = useState<WatchlistSummary[]>([]);
  const [form, setForm] = useState<WatchlistFormState>(emptyWatchlistForm);
  const [symbolInput, setSymbolInput] = useState("");
  const [isLoadingList, setIsLoadingList] = useState(true);
  const [isLoadingDetail, setIsLoadingDetail] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isSyncingSymbols, setIsSyncingSymbols] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [symbolNotice, setSymbolNotice] = useState<string | null>(null);
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  const [undoRemoval, setUndoRemoval] = useState<UndoRemoval | null>(null);
  const [deleteConfirmationId, setDeleteConfirmationId] = useState<string | null>(null);

  const detailRequestRef = useRef(0);
  const operationRef = useRef(false);
  const symbolSyncRef = useRef(false);
  const activeWatchlistIdRef = useRef<string | null>(null);
  const persistedMetaRef = useRef<PersistedMeta | null>(null);
  const persistedSymbolsRef = useRef<string[]>([]);
  const desiredSymbolsRef = useRef<string[]>([]);
  const undoTimerRef = useRef<number | null>(null);

  const isBlocking = isLoadingDetail || isSaving;
  const isBusy = isBlocking || isSyncingSymbols;
  const isConfirmingDelete = Boolean(form.id && deleteConfirmationId === form.id);
  const isInteractionLocked = isBusy || isConfirmingDelete;

  const clearUndo = useCallback(() => {
    if (undoTimerRef.current !== null) {
      window.clearTimeout(undoTimerRef.current);
      undoTimerRef.current = null;
    }
    setUndoRemoval(null);
  }, []);

  const applyPersistedDetail = useCallback(
    (detail: WatchlistDetail) => {
      activeWatchlistIdRef.current = detail.id;
      persistedMetaRef.current = detailToMeta(detail);
      persistedSymbolsRef.current = [...detail.symbols];
      desiredSymbolsRef.current = [...detail.symbols];
      setDeleteConfirmationId(null);
      setForm(detailToForm(detail));
      setSymbolInput("");
      setSymbolNotice(null);
      clearUndo();
    },
    [clearUndo],
  );

  const refreshList = useCallback(async () => {
    const rows = await listWatchlists();
    setWatchlists(rows);
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const rows = await listWatchlists();
        if (!cancelled) {
          setWatchlists(rows);
        }
      } catch (loadError) {
        if (!cancelled) {
          setError(formatAppError(loadError));
        }
      } finally {
        if (!cancelled) {
          setIsLoadingList(false);
        }
      }
    }

    void load();
    return () => {
      cancelled = true;
      operationRef.current = false;
      activeWatchlistIdRef.current = null;
      detailRequestRef.current += 1;
      if (undoTimerRef.current !== null) {
        window.clearTimeout(undoTimerRef.current);
      }
    };
  }, []);

  const selectWatchlist = useCallback(
    async (id: string) => {
      if (operationRef.current || symbolSyncRef.current) {
        return;
      }

      operationRef.current = true;
      const requestId = detailRequestRef.current + 1;
      detailRequestRef.current = requestId;
      setDeleteConfirmationId(null);
      setIsLoadingDetail(true);
      setError(null);
      setNotice(null);
      setSymbolNotice(null);
      setFieldErrors({});
      clearUndo();

      try {
        const detail = await getWatchlist(id);
        if (detailRequestRef.current === requestId) {
          applyPersistedDetail(detail);
        }
      } catch (loadError) {
        if (detailRequestRef.current === requestId) {
          setError(formatAppError(loadError));
        }
      } finally {
        operationRef.current = false;
        if (detailRequestRef.current === requestId) {
          setIsLoadingDetail(false);
        }
      }
    },
    [applyPersistedDetail, clearUndo],
  );

  const startNewWatchlist = useCallback(() => {
    if (operationRef.current || symbolSyncRef.current) {
      return;
    }

    detailRequestRef.current += 1;
    activeWatchlistIdRef.current = null;
    persistedMetaRef.current = null;
    persistedSymbolsRef.current = [];
    desiredSymbolsRef.current = [];
    setDeleteConfirmationId(null);
    setForm(emptyWatchlistForm());
    setSymbolInput("");
    setError(null);
    setNotice(null);
    setSymbolNotice(null);
    setFieldErrors({});
    clearUndo();
  }, [clearUndo]);

  const flushSymbolChanges = useCallback(async () => {
    if (symbolSyncRef.current) {
      return;
    }

    const watchlistId = activeWatchlistIdRef.current;
    if (!watchlistId || !persistedMetaRef.current) {
      return;
    }

    symbolSyncRef.current = true;
    setIsSyncingSymbols(true);
    setError(null);

    try {
      while (
        activeWatchlistIdRef.current === watchlistId &&
        !symbolsMatch(desiredSymbolsRef.current, persistedSymbolsRef.current)
      ) {
        const targetSymbols = [...desiredSymbolsRef.current];
        const meta = persistedMetaRef.current;

        if (!meta) {
          break;
        }

        try {
          const detail = await updateWatchlist(watchlistId, {
            name: meta.name,
            description: meta.description,
            symbols: targetSymbols,
          });

          if (activeWatchlistIdRef.current !== watchlistId) {
            return;
          }

          persistedMetaRef.current = detailToMeta(detail);
          persistedSymbolsRef.current = [...detail.symbols];
          setWatchlists((current) =>
            current.map((watchlist) =>
              watchlist.id === watchlistId
                ? {
                    ...watchlist,
                    name: detail.name,
                    description: detail.description,
                    symbolCount: detail.symbols.length,
                  }
                : watchlist,
            ),
          );

          if (symbolsMatch(desiredSymbolsRef.current, targetSymbols)) {
            desiredSymbolsRef.current = [...detail.symbols];
            setForm((current) =>
              current.id === watchlistId ? { ...current, symbols: [...detail.symbols] } : current,
            );
          }
        } catch (saveError) {
          if (activeWatchlistIdRef.current !== watchlistId) {
            return;
          }

          const rollbackSymbols = [...persistedSymbolsRef.current];
          desiredSymbolsRef.current = rollbackSymbols;
          setForm((current) =>
            current.id === watchlistId ? { ...current, symbols: rollbackSymbols } : current,
          );
          setWatchlists((current) =>
            current.map((watchlist) =>
              watchlist.id === watchlistId
                ? { ...watchlist, symbolCount: rollbackSymbols.length }
                : watchlist,
            ),
          );
          clearUndo();
          setError(`티커 변경을 저장하지 못했습니다. ${formatAppError(saveError)}`);
          break;
        }
      }
    } finally {
      symbolSyncRef.current = false;
      setIsSyncingSymbols(false);
    }
  }, [clearUndo]);

  function updateDesiredSymbols(nextSymbols: string[]) {
    desiredSymbolsRef.current = nextSymbols;
    setForm((current) => ({ ...current, symbols: nextSymbols }));

    if (activeWatchlistIdRef.current) {
      void flushSymbolChanges();
    }
  }

  function addPendingSymbols() {
    const result = addSymbols(form.symbols, symbolInput);
    const parsedAnything = result.added.length > 0 || result.duplicates.length > 0;

    if (!parsedAnything && result.omittedCount === 0) {
      setSymbolNotice("추가할 티커를 입력하십시오.");
      return;
    }

    setSymbolInput("");

    if (result.added.length > 0) {
      updateDesiredSymbols(result.symbols);
    }

    if (result.omittedCount > 0) {
      setSymbolNotice(
        `${MAX_WATCHLIST_SYMBOLS}종목 제한으로 ${result.omittedCount}개 티커를 추가하지 않았습니다.`,
      );
    } else if (result.duplicates.length > 0) {
      setSymbolNotice(`${result.duplicates.join(", ")}은(는) 이미 등록되어 있습니다.`);
    } else if (!form.id && result.added.length > 0) {
      setSymbolNotice("Watchlist를 생성하면 티커가 저장됩니다.");
    } else {
      setSymbolNotice(null);
    }
  }

  function handleSymbolKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key !== "Enter" || event.nativeEvent.isComposing) {
      return;
    }

    event.preventDefault();
    addPendingSymbols();
  }

  function removeTicker(symbol: string) {
    const index = form.symbols.indexOf(symbol);
    if (index < 0) {
      return;
    }

    const nextSymbols = removeSymbol(form.symbols, symbol);
    updateDesiredSymbols(nextSymbols);
    setSymbolNotice(null);

    if (undoTimerRef.current !== null) {
      window.clearTimeout(undoTimerRef.current);
    }

    setUndoRemoval({ symbol, index });
    undoTimerRef.current = window.setTimeout(() => {
      setUndoRemoval(null);
      undoTimerRef.current = null;
    }, 5_000);
  }

  function undoTickerRemoval() {
    if (!undoRemoval) {
      return;
    }

    const restoredSymbols = restoreSymbol(
      desiredSymbolsRef.current,
      undoRemoval.symbol,
      undoRemoval.index,
    );
    updateDesiredSymbols(restoredSymbols);
    setSymbolNotice(`${undoRemoval.symbol} 삭제를 취소했습니다.`);
    clearUndo();
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    if (operationRef.current || symbolSyncRef.current || deleteConfirmationId) {
      return;
    }

    setError(null);
    setNotice(null);
    setFieldErrors({});

    const name = form.name.trim();
    if (!name) {
      setFieldErrors({ name: "Watchlist 이름을 입력하십시오." });
      return;
    }

    operationRef.current = true;
    setIsSaving(true);
    const wasExisting = form.id !== null;

    try {
      const input = {
        name,
        description: form.description.trim() || null,
        symbols: form.symbols,
      };
      const detail = form.id
        ? await updateWatchlist(form.id, input)
        : await createWatchlist(input);

      applyPersistedDetail(detail);
      await refreshList();
      setNotice(wasExisting ? "이름과 설명을 저장했습니다." : "Watchlist를 생성했습니다.");
    } catch (saveError) {
      const payload = parseAppError(saveError);

      if (payload.code === "conflict" || payload.code === "validation") {
        setFieldErrors({ name: payload.message });
      } else {
        setError(formatAppError(saveError));
      }
    } finally {
      operationRef.current = false;
      setIsSaving(false);
    }
  }

  function requestDelete() {
    if (!form.id || operationRef.current || symbolSyncRef.current) {
      return;
    }

    setError(null);
    setNotice(null);
    setSymbolNotice(null);
    setFieldErrors({});
    clearUndo();
    setDeleteConfirmationId(form.id);
  }

  function cancelDelete() {
    if (!operationRef.current) {
      setDeleteConfirmationId(null);
    }
  }

  async function confirmDelete() {
    const watchlistId = form.id;
    if (
      !watchlistId ||
      deleteConfirmationId !== watchlistId ||
      operationRef.current ||
      symbolSyncRef.current
    ) {
      return;
    }

    operationRef.current = true;
    setIsSaving(true);
    setError(null);
    setNotice(null);
    setSymbolNotice(null);
    setFieldErrors({});
    clearUndo();

    try {
      await deleteWatchlist(watchlistId);
      activeWatchlistIdRef.current = null;
      persistedMetaRef.current = null;
      persistedSymbolsRef.current = [];
      desiredSymbolsRef.current = [];
      setDeleteConfirmationId(null);
      setForm(emptyWatchlistForm());
      setSymbolInput("");
      await refreshList();
      setNotice("Watchlist를 삭제했습니다.");
    } catch (deleteError) {
      setError(formatAppError(deleteError));
    } finally {
      operationRef.current = false;
      setIsSaving(false);
    }
  }

  return (
    <div className="watchlist-layout">
      <section className="panel watchlist-browser" aria-busy={isLoadingList || isBusy}>
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Saved groups</p>
            <h3>Watchlists</h3>
          </div>
          <button
            className="primary-button"
            type="button"
            onClick={startNewWatchlist}
            disabled={isInteractionLocked}
          >
            새 목록
          </button>
        </div>

        {isLoadingList ? <p className="muted">목록을 불러오는 중입니다.</p> : null}
        {!isLoadingList && !error && watchlists.length === 0 ? (
          <div className="compact-empty">
            <strong>저장된 Watchlist가 없습니다.</strong>
            <span>새 목록을 만들어 분석 대상을 구성하십시오.</span>
          </div>
        ) : null}

        <div className="watchlist-items">
          {watchlists.map((watchlist) => (
            <button
              key={watchlist.id}
              type="button"
              className={form.id === watchlist.id ? "watchlist-item active" : "watchlist-item"}
              onClick={() => void selectWatchlist(watchlist.id)}
              disabled={isInteractionLocked}
            >
              <span>
                <strong>{watchlist.name}</strong>
                <small>{watchlist.description || "설명 없음"}</small>
              </span>
              <b>{watchlist.symbolCount}</b>
            </button>
          ))}
        </div>
      </section>

      <section className="panel editor-panel" aria-busy={isBusy}>
        <div className="panel-heading">
          <div>
            <p className="eyebrow">{form.id ? "Edit Watchlist" : "New Watchlist"}</p>
            <h3>{form.id ? form.name || "Watchlist" : "새 Watchlist"}</h3>
          </div>
          {form.id && !isConfirmingDelete ? (
            <details className={styles.watchlistMenu}>
              <summary aria-label="Watchlist 메뉴">•••</summary>
              <div className={styles.watchlistMenuPopover}>
                <button
                  className={styles.menuDangerButton}
                  type="button"
                  disabled={isBusy}
                  onClick={requestDelete}
                >
                  Watchlist 삭제
                </button>
              </div>
            </details>
          ) : null}
        </div>

        {isConfirmingDelete ? (
          <div className="message error-message" role="alert" aria-live="assertive">
            <strong>“{form.name}” Watchlist를 삭제하시겠습니까?</strong>
            <p>삭제한 Watchlist는 복구할 수 없습니다.</p>
            <div className="form-actions">
              <button
                className="secondary-button"
                type="button"
                onClick={cancelDelete}
                disabled={isSaving}
              >
                취소
              </button>
              <button
                className="danger-button"
                type="button"
                onClick={() => void confirmDelete()}
                disabled={isSaving}
              >
                {isSaving ? "삭제 중…" : "삭제 확인"}
              </button>
            </div>
          </div>
        ) : null}

        <form
          className="watchlist-form"
          onSubmit={(event: FormEvent<HTMLFormElement>) => void submit(event)}
        >
          <label>
            <span>이름</span>
            <input
              value={form.name}
              maxLength={80}
              disabled={isInteractionLocked}
              placeholder="예: 전체 종목, 빅테크, 장기 매수 후보"
              onChange={(event: ChangeEvent<HTMLInputElement>) =>
                setForm((current) => ({ ...current, name: event.target.value }))
              }
            />
          </label>
          {fieldErrors.name ? <div className="field-error">{fieldErrors.name}</div> : null}

          <label>
            <span>설명</span>
            <input
              value={form.description}
              maxLength={500}
              disabled={isInteractionLocked}
              placeholder="선택 사항"
              onChange={(event: ChangeEvent<HTMLInputElement>) =>
                setForm((current) => ({ ...current, description: event.target.value }))
              }
            />
          </label>

          <section className={styles.symbolManager} aria-label="티커 관리">
            <div className={styles.symbolManagerHeading}>
              <div>
                <strong>티커</strong>
                <small>Enter로 추가하며 쉼표·공백으로 여러 종목을 붙여넣을 수 있습니다.</small>
              </div>
              <span className={styles.symbolSaveStatus}>
                {isSyncingSymbols ? "저장 중…" : form.id ? "저장됨" : "목록 생성 시 저장"}
              </span>
            </div>

            <div className={styles.symbolAddRow}>
              <input
                aria-label="추가할 티커"
                value={symbolInput}
                disabled={
                  isBlocking || isConfirmingDelete || form.symbols.length >= MAX_WATCHLIST_SYMBOLS
                }
                placeholder="예: AAPL 또는 AAPL, MSFT, NVDA"
                autoCapitalize="characters"
                spellCheck={false}
                onChange={(event: ChangeEvent<HTMLInputElement>) =>
                  setSymbolInput(event.target.value.toUpperCase())
                }
                onKeyDown={handleSymbolKeyDown}
              />
              <button
                className="primary-button"
                type="button"
                disabled={
                  isBlocking ||
                  isConfirmingDelete ||
                  form.symbols.length >= MAX_WATCHLIST_SYMBOLS ||
                  symbolInput.trim().length === 0
                }
                onClick={addPendingSymbols}
              >
                추가
              </button>
            </div>

            {symbolNotice ? <div className={styles.symbolNotice}>{symbolNotice}</div> : null}

            {form.symbols.length > 0 ? (
              <div className={styles.symbolList} role="list" aria-label="등록된 티커">
                {form.symbols.map((symbol) => (
                  <div className={styles.symbolRow} role="listitem" key={symbol}>
                    <strong>{symbol}</strong>
                    <button
                      type="button"
                      aria-label={`${symbol} 삭제`}
                      disabled={isBlocking || isConfirmingDelete}
                      onClick={() => removeTicker(symbol)}
                    >
                      ×
                    </button>
                  </div>
                ))}
              </div>
            ) : (
              <div className={styles.symbolEmpty}>
                등록된 티커가 없습니다. 위 입력창에서 바로 추가하십시오.
              </div>
            )}

            <div className="form-meta">
              <span>
                {form.symbols.length} / {MAX_WATCHLIST_SYMBOLS}종목
              </span>
              <span>티커 추가·삭제는 자동 저장</span>
            </div>
          </section>

          {undoRemoval ? (
            <div className={styles.undoMessage} role="status">
              <span>{undoRemoval.symbol}을(를) 제거했습니다.</span>
              <button type="button" onClick={undoTickerRemoval}>
                실행 취소
              </button>
            </div>
          ) : null}

          {error ? <div className="message error-message">{error}</div> : null}
          {notice ? <div className="message success-message">{notice}</div> : null}

          <div className="form-actions">
            <button
              className="secondary-button"
              type="button"
              disabled={isInteractionLocked}
              onClick={startNewWatchlist}
            >
              초기화
            </button>
            <button
              className="primary-button strong"
              type="submit"
              disabled={isInteractionLocked || form.symbols.length > MAX_WATCHLIST_SYMBOLS}
            >
              {isSaving ? "저장 중…" : form.id ? "이름·설명 저장" : "Watchlist 생성"}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}
