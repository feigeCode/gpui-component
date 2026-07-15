<template>
    <aside class="migration-notice" aria-labelledby="migration-title">
        <div class="notice-copy">
            <p class="notice-kicker">项目迁移 / Project migration</p>
            <h2 id="migration-title">OnetCli 已迁移至 Navop</h2>
            <p>
                OnetCli 已停止更新，但本仓库暂不归档。历史代码和现有 Issues 将继续保留；
                OnetCli 后续不再发布新功能、问题修复或新版本，相关问题将在新的 Navop 仓库中继续修复。
            </p>
            <p>
                OnetCli is no longer updated, but this repository is not archived for now.
                Historical code and existing issues remain available; future fixes will be made in the new Navop repository.
            </p>
        </div>

        <div class="notice-links" aria-label="Navop migration links">
            <a href="https://navop.dev" target="_blank" rel="noreferrer">
                <span>Navop 官方网站 / Official website</span>
                <strong>https://navop.dev</strong>
            </a>
            <a href="https://github.com/feigeCode/navop" target="_blank" rel="noreferrer">
                <span>Navop GitHub 仓库 / Repository</span>
                <strong>https://github.com/feigeCode/navop</strong>
            </a>
        </div>
    </aside>
</template>

<style scoped lang="scss">
.migration-notice {
    position: relative;
    z-index: 1;
    display: grid;
    grid-template-columns: minmax(0, 1.15fr) minmax(300px, 0.85fr);
    gap: 24px;
    margin-bottom: 48px;
    border: 1px solid rgba(240, 139, 121, 0.42);
    border-radius: 20px;
    padding: 26px;
    background:
        radial-gradient(circle at 0 0, rgba(224, 90, 71, 0.15), transparent 24rem),
        rgba(19, 22, 34, 0.94);
    box-shadow: 0 26px 90px rgba(0, 0, 0, 0.38);
}

.notice-kicker {
    margin: 0 0 10px;
    color: #f08b79;
    font-size: 13px;
    font-weight: 760;
    letter-spacing: 0.04em;
    text-transform: uppercase;
}

h2 {
    margin: 0 0 14px;
    color: #ffffff;
    font-size: clamp(24px, 3vw, 36px);
    line-height: 1.12;
}

.notice-copy p:not(.notice-kicker) {
    margin: 0;
    color: #c6ccda;
    font-size: 15px;
    line-height: 1.72;

    & + p {
        margin-top: 10px;
        color: #9da6b8;
    }
}

.notice-links {
    display: grid;
    align-content: center;
    gap: 12px;

    a {
        display: block;
        border: 1px solid rgba(240, 139, 121, 0.24);
        border-radius: 14px;
        padding: 15px 16px;
        background: rgba(11, 13, 19, 0.72);
        text-decoration: none !important;
        transition: border-color 0.2s ease, transform 0.2s ease;

        &:hover {
            transform: translateY(-1px);
            border-color: rgba(240, 139, 121, 0.58);
        }
    }

    span,
    strong {
        display: block;
    }

    span {
        margin-bottom: 5px;
        color: #9da6b8;
        font-size: 12px;
    }

    strong {
        color: #ffffff;
        font-size: 14px;
        overflow-wrap: anywhere;
    }
}

@media (max-width: 920px) {
    .migration-notice {
        grid-template-columns: 1fr;
        margin-bottom: 36px;
        padding: 20px;
    }
}
</style>
