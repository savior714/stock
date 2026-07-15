"use client";

import { FormEvent, useCallback, useEffect, useMemo, useState } from "react";

import {
  createWatchlist,
  deleteWatchlist,
  errorMessage,
  getWatchlist,
  listWatchlists,
  parseSymbols,
  updateWatchlist,
} from "./api";
import {
  emptyWatchlistForm,
  type WatchlistDetail,
  type WatchlistFormState,
  type WatchlistSummary,
} from "./types";

function detailToForm(detail: WatchlistDetail): WatchlistFormState {
  return {
    id: detail.id,
    name: detail.name,
    description: detail.description ?? "",
    symbolsText: detail.symbols.join("\n"),
  };
}

export default function WatchlistWorkspace() {
  const [watchlists, setWatchlists] = useState<WatchlistSummary[]>([]);
  const [form, setForm] = useState<WatchlistFormState>(emptyWatchlistForm);
  const [isLoadingList, setIsLoadingList] = useState(true);
  const [isLoadingDetail, setIsLoadingDetail] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const symbols = useMemo(() => parseSymbols(form.symbolsText), [form.symbolsText]);

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
          setError(errorMessage(loadError));
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
    };
  }, []);

  async function selectWatchlist(id: string) {
    setIsLoadingDetail(true);
    setError(null);
    setNotice(null);

    try {
      const detail = await getWatchlist(id);
      setForm(detailToForm(detail));
    } catch (loadError) {
      setError(errorMessage(loadError));
    } finally {
      setIsLoadingDetail(false);
    }
  }

  function startNewWatchlist() {
    setForm(emptyWatchlistForm());
    setError(null);
    setNotice(null);
  }

  async function submit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setError(null);
    setNotice(null);

    const name = form.name.trim();
    if (!name) {
      setError("Watchlist 이름을 입력하십시오.");
      return;
    }

    setIsSaving(true);
    try {
      const input = {
        name,
        description: form.description.trim() || null,
        symbols,
      };
      const detail = form.id
        ? await updateWatchlist(form.id, input)
        : await createWatchlist(input);

      setForm(detailToForm(detail));
      await refreshList();
      setNotice(form.id ? "Watchlist를 수정했습니다." : "Watchlist를 생성했습니다.");
    } catch (saveError) {
      setError(errorMessage(saveError));
    } finally {
      setIsSaving(false);
    }
  }

  async function removeSelected() {
    if (!form.id) {
      return;
    }
    if (!window.confirm(`“${form.name}” Watchlist를 삭제하시겠습니까?`)) {
      return;
    }

    setIsSaving(true);
    setError(null);
    setNotice(null);
    try {
      await deleteWatchlist(form.id);
      setForm(emptyWatchlistForm());
      await refreshList();
      setNotice("Watchlist를 삭제했습니다.");
    } catch (deleteError) {
      setError(errorMessage(deleteError));
    } finally {
      setIsSaving(false);
    }
  }

  return (
    <div className="watchlist-layout">
      <section className="panel watchlist-browser" aria-busy={isLoadingList}>
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Saved groups</p>
            <h3>Watchlists</h3>
          </div>
          <button className="primary-button" type="button" onClick={startNewWatchlist}>
            새 목록
          </button>
        </div>

        {isLoadingList ? <p className="muted">목록을 불러오는 중입니다.</p> : null}
        {!isLoadingList && watchlists.length === 0 ? (
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

      <section className="panel editor-panel" aria-busy={isLoadingDetail || isSaving}>
        <div className="panel-heading">
          <div>
            <p className="eyebrow">{form.id ? "Edit Watchlist" : "New Watchlist"}</p>
            <h3>{form.id ? form.name || "Watchlist" : "새 Watchlist"}</h3>
          </div>
          {form.id ? (
            <button
              className="danger-button"
              type="button"
              disabled={isSaving}
              onClick={() => void removeSelected()}
            >
              삭제
            </button>
          ) : null}
        </div>

        <form className="watchlist-form" onSubmit={(event) => void submit(event)}>
          <label>
            <span>이름</span>
            <input
              value={form.name}
              maxLength={80}
              disabled={isLoadingDetail || isSaving}
              placeholder="예: 전체 종목, 빅테크, 장기 매수 후보"
              onChange={(event) => setForm((current) => ({ ...current, name: event.target.value }))}
            />
          </label>

          <label>
            <span>설명</span>
            <input
              value={form.description}
              maxLength={500}
              disabled={isLoadingDetail || isSaving}
              placeholder="선택 사항"
              onChange={(event) =>
                setForm((current) => ({ ...current, description: event.target.value }))
              }
            />
          </label>

          <label className="symbols-field">
            <span>티커</span>
            <textarea
              value={form.symbolsText}
              disabled={isLoadingDetail || isSaving}
              placeholder={"AAPL\nMSFT\nNVDA\nQQQ"}
              onChange={(event) =>
                setForm((current) => ({ ...current, symbolsText: event.target.value }))
              }
            />
          </label>

          <div className="form-meta">
            <span>{symbols.length} / 500종목</span>
            <span>쉼표, 공백 또는 줄바꿈으로 구분</span>
          </div>

          {error ? <div className="message error-message">{error}</div> : null}
          {notice ? <div className="message success-message">{notice}</div> : null}

          <div className="form-actions">
            <button
              className="secondary-button"
              type="button"
              disabled={isSaving}
              onClick={startNewWatchlist}
            >
              초기화
            </button>
            <button
              className="primary-button strong"
              type="submit"
              disabled={isLoadingDetail || isSaving || symbols.length > 500}
            >
              {isSaving ? "저장 중…" : form.id ? "변경 저장" : "Watchlist 생성"}
            </button>
          </div>
        </form>
      </section>
    </div>
  );
}
