import { ElmType, Elm, u64 } from "./0_core";

export class ElmAccountIndex implements Elm {
    static readonly index: ElmType = ElmType.Index;
    readonly index: ElmType = ElmAccountIndex.index;
    readonly id: u64;
    // other fields
}

export class ElmAccount implements Elm {
    static readonly index: ElmType = ElmType.Account;
    readonly index: ElmType = ElmAccount.index;
    readonly id: u64;
    // other fields
}

export class ElmAccountFactorNormal implements Elm {
    static readonly index: ElmType = ElmType.AccountFactorNormal;
    readonly index: ElmType = ElmAccountFactorNormal.index;
    readonly id: u64;
    // other fields
    username: string
    password_hash: string
    password_salt: bigint
}

export class ElmAccountFactorCrypto implements Elm {
    static readonly index: ElmType = ElmType.AccountFactorCrypto;
    readonly index: ElmType = ElmAccountFactorCrypto.index;
    readonly id: u64;
    // other fields
    public_key: string
    private_key_locked: string
}

export class ElmAsset implements Elm {
    static readonly index: ElmType = ElmType.Asset;
    readonly index: ElmType = ElmAsset.index;
    readonly id: u64
    // other fields
    symbol: string
    decimals: u64
    // if true, requires two-way handshake for payment
    liability: boolean
}

export class ElmBalance implements Elm {
    static readonly index: ElmType = ElmType.Balance;
    readonly index: ElmType = ElmBalance.index;
    readonly id: u64
    // other fields
    account: u64
    asset: u64
    amount: u64
}

export class ElmPayment implements Elm {
    static readonly index: ElmType = ElmType.Balance;
    readonly index: ElmType = ElmBalance.index;
    readonly id: u64
    // other fields
    unk_account: u64
    src_account: u64
    tar_account: u64
    asset: u64
    amount: u64
}

export class ElmExchange implements Elm {
    static readonly index: ElmType = ElmType.Exchange;
    readonly index: ElmType = ElmExchange.index;
    readonly id: u64
    // other fields
    payments: [ElmPayment]
}