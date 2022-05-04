use super::*;
use crate::mm2::lp_swap::max_taker_vol_from_available;
use bigdecimal::BigDecimal;
use coins::qrc20::rpc_clients::for_tests::Qrc20NativeWalletOps;
use coins::qrc20::{qrc20_coin_from_conf_and_request, Qrc20Coin};
use coins::utxo::qtum::QtumBasedCoin;
use coins::utxo::qtum::{qtum_coin_from_conf_and_request, QtumCoin};
use coins::utxo::sat_from_big_decimal;
use coins::utxo::utxo_common::big_decimal_from_sat;
use coins::{FeeApproxStage, MarketCoinOps, MmCoin, TradePreimageValue, TransactionEnum};
use common::for_tests::{check_my_swap_status, check_recent_swaps, check_stats_swap_status, MAKER_ERROR_EVENTS,
                        MAKER_SUCCESS_EVENTS, TAKER_ERROR_EVENTS, TAKER_SUCCESS_EVENTS};
use common::log::debug;
use common::mm_ctx::MmArc;
use common::{temp_dir, DEX_FEE_ADDR_RAW_PUBKEY};
use ethereum_types::H160;
use http::StatusCode;
use rand6::Rng;
use std::path::PathBuf;
use std::str::FromStr;

pub const QTUM_REGTEST_DOCKER_IMAGE: &str = "sergeyboyko/qtumregtest";

const QRC20_TOKEN_BYTES: &str = "6080604052600860ff16600a0a633b9aca000260005534801561002157600080fd5b50600054600160003373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002081905550610c69806100776000396000f3006080604052600436106100a4576000357c0100000000000000000000000000000000000000000000000000000000900463ffffffff16806306fdde03146100a9578063095ea7b31461013957806318160ddd1461019e57806323b872dd146101c9578063313ce5671461024e5780635a3b7e421461027f57806370a082311461030f57806395d89b4114610366578063a9059cbb146103f6578063dd62ed3e1461045b575b600080fd5b3480156100b557600080fd5b506100be6104d2565b6040518080602001828103825283818151815260200191508051906020019080838360005b838110156100fe5780820151818401526020810190506100e3565b50505050905090810190601f16801561012b5780820380516001836020036101000a031916815260200191505b509250505060405180910390f35b34801561014557600080fd5b50610184600480360381019080803573ffffffffffffffffffffffffffffffffffffffff1690602001909291908035906020019092919050505061050b565b604051808215151515815260200191505060405180910390f35b3480156101aa57600080fd5b506101b36106bb565b6040518082815260200191505060405180910390f35b3480156101d557600080fd5b50610234600480360381019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803590602001909291905050506106c1565b604051808215151515815260200191505060405180910390f35b34801561025a57600080fd5b506102636109a1565b604051808260ff1660ff16815260200191505060405180910390f35b34801561028b57600080fd5b506102946109a6565b6040518080602001828103825283818151815260200191508051906020019080838360005b838110156102d45780820151818401526020810190506102b9565b50505050905090810190601f1680156103015780820380516001836020036101000a031916815260200191505b509250505060405180910390f35b34801561031b57600080fd5b50610350600480360381019080803573ffffffffffffffffffffffffffffffffffffffff1690602001909291905050506109df565b6040518082815260200191505060405180910390f35b34801561037257600080fd5b5061037b6109f7565b6040518080602001828103825283818151815260200191508051906020019080838360005b838110156103bb5780820151818401526020810190506103a0565b50505050905090810190601f1680156103e85780820380516001836020036101000a031916815260200191505b509250505060405180910390f35b34801561040257600080fd5b50610441600480360381019080803573ffffffffffffffffffffffffffffffffffffffff16906020019092919080359060200190929190505050610a30565b604051808215151515815260200191505060405180910390f35b34801561046757600080fd5b506104bc600480360381019080803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803573ffffffffffffffffffffffffffffffffffffffff169060200190929190505050610be1565b6040518082815260200191505060405180910390f35b6040805190810160405280600881526020017f515243205445535400000000000000000000000000000000000000000000000081525081565b60008260008173ffffffffffffffffffffffffffffffffffffffff161415151561053457600080fd5b60008314806105bf57506000600260003373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060008673ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002054145b15156105ca57600080fd5b82600260003373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060008673ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff168152602001908152602001600020819055508373ffffffffffffffffffffffffffffffffffffffff163373ffffffffffffffffffffffffffffffffffffffff167f8c5be1e5ebec7d5bd14f71427d1e84f3dd0314c0f7b2291e5b200ac8c7c3b925856040518082815260200191505060405180910390a3600191505092915050565b60005481565b60008360008173ffffffffffffffffffffffffffffffffffffffff16141515156106ea57600080fd5b8360008173ffffffffffffffffffffffffffffffffffffffff161415151561071157600080fd5b610797600260008873ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060003373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020016000205485610c06565b600260008873ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002060003373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002081905550610860600160008873ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020016000205485610c06565b600160008873ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff168152602001908152602001600020819055506108ec600160008773ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020016000205485610c1f565b600160008773ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff168152602001908152602001600020819055508473ffffffffffffffffffffffffffffffffffffffff168673ffffffffffffffffffffffffffffffffffffffff167fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef866040518082815260200191505060405180910390a36001925050509392505050565b600881565b6040805190810160405280600981526020017f546f6b656e20302e31000000000000000000000000000000000000000000000081525081565b60016020528060005260406000206000915090505481565b6040805190810160405280600381526020017f515443000000000000000000000000000000000000000000000000000000000081525081565b60008260008173ffffffffffffffffffffffffffffffffffffffff1614151515610a5957600080fd5b610aa2600160003373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020016000205484610c06565b600160003373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200190815260200160002081905550610b2e600160008673ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020019081526020016000205484610c1f565b600160008673ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff168152602001908152602001600020819055508373ffffffffffffffffffffffffffffffffffffffff163373ffffffffffffffffffffffffffffffffffffffff167fddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef856040518082815260200191505060405180910390a3600191505092915050565b6002602052816000526040600020602052806000526040600020600091509150505481565b6000818310151515610c1457fe5b818303905092915050565b6000808284019050838110151515610c3357fe5b80915050929150505600a165627a7a723058207f2e5248b61b80365ea08a0f6d11ac0b47374c4dfd538de76bc2f19591bbbba40029";
const QRC20_SWAP_CONTRACT_BYTES: &str = "608060405234801561001057600080fd5b50611437806100206000396000f3fe60806040526004361061004a5760003560e01c806302ed292b1461004f5780630716326d146100de578063152cf3af1461017b57806346fc0294146101f65780639b415b2a14610294575b600080fd5b34801561005b57600080fd5b506100dc600480360360a081101561007257600080fd5b81019080803590602001909291908035906020019092919080359060200190929190803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803573ffffffffffffffffffffffffffffffffffffffff169060200190929190505050610339565b005b3480156100ea57600080fd5b506101176004803603602081101561010157600080fd5b8101908080359060200190929190505050610867565b60405180846bffffffffffffffffffffffff19166bffffffffffffffffffffffff191681526020018367ffffffffffffffff1667ffffffffffffffff16815260200182600381111561016557fe5b60ff168152602001935050505060405180910390f35b6101f46004803603608081101561019157600080fd5b8101908080359060200190929190803573ffffffffffffffffffffffffffffffffffffffff16906020019092919080356bffffffffffffffffffffffff19169060200190929190803567ffffffffffffffff1690602001909291905050506108bf565b005b34801561020257600080fd5b50610292600480360360a081101561021957600080fd5b81019080803590602001909291908035906020019092919080356bffffffffffffffffffffffff19169060200190929190803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803573ffffffffffffffffffffffffffffffffffffffff169060200190929190505050610bd9565b005b610337600480360360c08110156102aa57600080fd5b810190808035906020019092919080359060200190929190803573ffffffffffffffffffffffffffffffffffffffff169060200190929190803573ffffffffffffffffffffffffffffffffffffffff16906020019092919080356bffffffffffffffffffffffff19169060200190929190803567ffffffffffffffff169060200190929190505050610fe2565b005b6001600381111561034657fe5b600080878152602001908152602001600020600001601c9054906101000a900460ff16600381111561037457fe5b1461037e57600080fd5b6000600333836003600288604051602001808281526020019150506040516020818303038152906040526040518082805190602001908083835b602083106103db57805182526020820191506020810190506020830392506103b8565b6001836020036101000a038019825116818451168082178552505050505050905001915050602060405180830381855afa15801561041d573d6000803e3d6000fd5b5050506040513d602081101561043257600080fd5b8101908080519060200190929190505050604051602001808281526020019150506040516020818303038152906040526040518082805190602001908083835b602083106104955780518252602082019150602081019050602083039250610472565b6001836020036101000a038019825116818451168082178552505050505050905001915050602060405180830381855afa1580156104d7573d6000803e3d6000fd5b5050506040515160601b8689604051602001808673ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1660601b81526014018573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1660601b8152601401846bffffffffffffffffffffffff19166bffffffffffffffffffffffff191681526014018373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1660601b8152601401828152602001955050505050506040516020818303038152906040526040518082805190602001908083835b602083106105fc57805182526020820191506020810190506020830392506105d9565b6001836020036101000a038019825116818451168082178552505050505050905001915050602060405180830381855afa15801561063e573d6000803e3d6000fd5b5050506040515160601b905060008087815260200190815260200160002060000160009054906101000a900460601b6bffffffffffffffffffffffff1916816bffffffffffffffffffffffff19161461069657600080fd5b6002600080888152602001908152602001600020600001601c6101000a81548160ff021916908360038111156106c857fe5b0217905550600073ffffffffffffffffffffffffffffffffffffffff168373ffffffffffffffffffffffffffffffffffffffff16141561074e573373ffffffffffffffffffffffffffffffffffffffff166108fc869081150290604051600060405180830381858888f19350505050158015610748573d6000803e3d6000fd5b50610820565b60008390508073ffffffffffffffffffffffffffffffffffffffff1663a9059cbb33886040518363ffffffff1660e01b8152600401808373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200182815260200192505050602060405180830381600087803b1580156107da57600080fd5b505af11580156107ee573d6000803e3d6000fd5b505050506040513d602081101561080457600080fd5b810190808051906020019092919050505061081e57600080fd5b505b7f36c177bcb01c6d568244f05261e2946c8c977fa50822f3fa098c470770ee1f3e8685604051808381526020018281526020019250505060405180910390a1505050505050565b60006020528060005260406000206000915090508060000160009054906101000a900460601b908060000160149054906101000a900467ffffffffffffffff169080600001601c9054906101000a900460ff16905083565b600073ffffffffffffffffffffffffffffffffffffffff168373ffffffffffffffffffffffffffffffffffffffff16141580156108fc5750600034115b801561094057506000600381111561091057fe5b600080868152602001908152602001600020600001601c9054906101000a900460ff16600381111561093e57fe5b145b61094957600080fd5b60006003843385600034604051602001808673ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1660601b81526014018573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1660601b8152601401846bffffffffffffffffffffffff19166bffffffffffffffffffffffff191681526014018373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1660601b8152601401828152602001955050505050506040516020818303038152906040526040518082805190602001908083835b60208310610a6c5780518252602082019150602081019050602083039250610a49565b6001836020036101000a038019825116818451168082178552505050505050905001915050602060405180830381855afa158015610aae573d6000803e3d6000fd5b5050506040515160601b90506040518060600160405280826bffffffffffffffffffffffff191681526020018367ffffffffffffffff16815260200160016003811115610af757fe5b81525060008087815260200190815260200160002060008201518160000160006101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908360601c021790555060208201518160000160146101000a81548167ffffffffffffffff021916908367ffffffffffffffff160217905550604082015181600001601c6101000a81548160ff02191690836003811115610b9357fe5b02179055509050507fccc9c05183599bd3135da606eaaf535daffe256e9de33c048014cffcccd4ad57856040518082815260200191505060405180910390a15050505050565b60016003811115610be657fe5b600080878152602001908152602001600020600001601c9054906101000a900460ff166003811115610c1457fe5b14610c1e57600080fd5b600060038233868689604051602001808673ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1660601b81526014018573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1660601b8152601401846bffffffffffffffffffffffff19166bffffffffffffffffffffffff191681526014018373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1660601b8152601401828152602001955050505050506040516020818303038152906040526040518082805190602001908083835b60208310610d405780518252602082019150602081019050602083039250610d1d565b6001836020036101000a038019825116818451168082178552505050505050905001915050602060405180830381855afa158015610d82573d6000803e3d6000fd5b5050506040515160601b905060008087815260200190815260200160002060000160009054906101000a900460601b6bffffffffffffffffffffffff1916816bffffffffffffffffffffffff1916148015610e10575060008087815260200190815260200160002060000160149054906101000a900467ffffffffffffffff1667ffffffffffffffff164210155b610e1957600080fd5b6003600080888152602001908152602001600020600001601c6101000a81548160ff02191690836003811115610e4b57fe5b0217905550600073ffffffffffffffffffffffffffffffffffffffff168373ffffffffffffffffffffffffffffffffffffffff161415610ed1573373ffffffffffffffffffffffffffffffffffffffff166108fc869081150290604051600060405180830381858888f19350505050158015610ecb573d6000803e3d6000fd5b50610fa3565b60008390508073ffffffffffffffffffffffffffffffffffffffff1663a9059cbb33886040518363ffffffff1660e01b8152600401808373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff16815260200182815260200192505050602060405180830381600087803b158015610f5d57600080fd5b505af1158015610f71573d6000803e3d6000fd5b505050506040513d6020811015610f8757600080fd5b8101908080519060200190929190505050610fa157600080fd5b505b7f1797d500133f8e427eb9da9523aa4a25cb40f50ebc7dbda3c7c81778973f35ba866040518082815260200191505060405180910390a1505050505050565b600073ffffffffffffffffffffffffffffffffffffffff168373ffffffffffffffffffffffffffffffffffffffff161415801561101f5750600085115b801561106357506000600381111561103357fe5b600080888152602001908152602001600020600001601c9054906101000a900460ff16600381111561106157fe5b145b61106c57600080fd5b60006003843385888a604051602001808673ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1660601b81526014018573ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1660601b8152601401846bffffffffffffffffffffffff19166bffffffffffffffffffffffff191681526014018373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1660601b8152601401828152602001955050505050506040516020818303038152906040526040518082805190602001908083835b6020831061118e578051825260208201915060208101905060208303925061116b565b6001836020036101000a038019825116818451168082178552505050505050905001915050602060405180830381855afa1580156111d0573d6000803e3d6000fd5b5050506040515160601b90506040518060600160405280826bffffffffffffffffffffffff191681526020018367ffffffffffffffff1681526020016001600381111561121957fe5b81525060008089815260200190815260200160002060008201518160000160006101000a81548173ffffffffffffffffffffffffffffffffffffffff021916908360601c021790555060208201518160000160146101000a81548167ffffffffffffffff021916908367ffffffffffffffff160217905550604082015181600001601c6101000a81548160ff021916908360038111156112b557fe5b021790555090505060008590508073ffffffffffffffffffffffffffffffffffffffff166323b872dd33308a6040518463ffffffff1660e01b8152600401808473ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020018373ffffffffffffffffffffffffffffffffffffffff1673ffffffffffffffffffffffffffffffffffffffff1681526020018281526020019350505050602060405180830381600087803b15801561137d57600080fd5b505af1158015611391573d6000803e3d6000fd5b505050506040513d60208110156113a757600080fd5b81019080805190602001909291905050506113c157600080fd5b7fccc9c05183599bd3135da606eaaf535daffe256e9de33c048014cffcccd4ad57886040518082815260200191505060405180910390a1505050505050505056fea265627a7a723158208c83db436905afce0b7be1012be64818c49323c12d451fe2ab6bce76ff6421c964736f6c63430005110032";
const QTUM_ADDRESS_LABEL: &str = "MM2_ADDRESS_LABEL";

static mut QICK_TOKEN_ADDRESS: Option<H160> = None;
static mut QORTY_TOKEN_ADDRESS: Option<H160> = None;
static mut QRC20_SWAP_CONTRACT_ADDRESS: Option<H160> = None;
static mut QTUM_CONF_PATH: Option<PathBuf> = None;

pub struct QtumDockerOps {
    #[allow(dead_code)]
    ctx: MmArc,
    coin: QtumCoin,
}

impl CoinDockerOps for QtumDockerOps {
    fn rpc_client(&self) -> &UtxoRpcClientEnum { &self.coin.as_ref().rpc_client }
}

impl QtumDockerOps {
    pub fn new() -> QtumDockerOps {
        let ctx = MmCtxBuilder::new().into_mm_arc();
        let confpath = unsafe { QTUM_CONF_PATH.as_ref().expect("Qtum config is not set yet") };
        let conf = json!({"decimals":8,"network":"regtest","confpath":confpath});
        let req = json!({
            "method": "enable",
        });
        let priv_key = hex::decode("809465b17d0a4ddb3e4c69e8f23c2cabad868f51f8bed5c765ad1d6516c3306f").unwrap();
        let coin = block_on(qtum_coin_from_conf_and_request(&ctx, "QTUM", &conf, &req, &priv_key)).unwrap();
        QtumDockerOps { ctx, coin }
    }

    pub fn initialize_contracts(&self) {
        let sender = get_address_by_label(&self.coin, QTUM_ADDRESS_LABEL);
        unsafe {
            QICK_TOKEN_ADDRESS = Some(self.create_contract(&sender, QRC20_TOKEN_BYTES));
            QORTY_TOKEN_ADDRESS = Some(self.create_contract(&sender, QRC20_TOKEN_BYTES));
            QRC20_SWAP_CONTRACT_ADDRESS = Some(self.create_contract(&sender, QRC20_SWAP_CONTRACT_BYTES));
        }
    }

    fn create_contract(&self, sender: &str, hexbytes: &str) -> H160 {
        let bytecode = hex::decode(hexbytes).expect("Hex encoded bytes expected");
        let gas_limit = 2_500_000u64;
        let gas_price = BigDecimal::from_str("0.0000004").unwrap();

        match self.coin.as_ref().rpc_client {
            UtxoRpcClientEnum::Native(ref native) => {
                let result = native
                    .create_contract(&bytecode.into(), gas_limit, gas_price, sender)
                    .wait()
                    .expect("!createcontract");
                result.address.0.into()
            },
            UtxoRpcClientEnum::Electrum(_) => panic!("Native client expected"),
        }
    }
}

pub fn qtum_docker_node(docker: &Cli, port: u16) -> UtxoDockerNode {
    let args = vec!["-p".into(), format!("127.0.0.1:{}:{}", port, port)];
    let image = GenericImage::new(QTUM_REGTEST_DOCKER_IMAGE)
        .with_args(args)
        .with_env_var("CLIENTS", "2")
        .with_env_var("COIN_RPC_PORT", port.to_string())
        .with_env_var("ADDRESS_LABEL", QTUM_ADDRESS_LABEL)
        .with_env_var("FILL_MEMPOOL", "true")
        .with_wait_for(WaitFor::message_on_stdout("config is ready"));
    let container = docker.run(image);

    let name = "qtum";
    let mut conf_path = temp_dir().join("qtum-regtest");
    std::fs::create_dir_all(&conf_path).unwrap();
    conf_path.push(format!("{}.conf", name));
    Command::new("docker")
        .arg("cp")
        .arg(format!("{}:/data/node_0/{}.conf", container.id(), name))
        .arg(&conf_path)
        .status()
        .expect("Failed to execute docker command");
    let timeout = now_ms() + 3000;
    loop {
        if conf_path.exists() {
            break;
        };
        assert!(now_ms() < timeout, "Test timed out");
    }

    unsafe { QTUM_CONF_PATH = Some(conf_path) };
    UtxoDockerNode {
        container,
        ticker: name.to_owned(),
        port,
    }
}

/// Build `Qrc20Coin` from ticker and privkey without filling the balance.
fn qrc20_coin_from_privkey(ticker: &str, priv_key: &[u8]) -> (MmArc, Qrc20Coin) {
    let (contract_address, swap_contract_address) = unsafe {
        let contract_address = match ticker {
            "QICK" => QICK_TOKEN_ADDRESS.expect("QICK_TOKEN_ADDRESS must be set already"),
            "QORTY" => QORTY_TOKEN_ADDRESS.expect("QORTY_TOKEN_ADDRESS must be set already"),
            _ => panic!("Expected QICK or QORTY ticker"),
        };
        (
            contract_address,
            QRC20_SWAP_CONTRACT_ADDRESS.expect("QRC20_SWAP_CONTRACT_ADDRESS must be set already"),
        )
    };
    let platform = "QTUM";
    let ctx = MmCtxBuilder::new().into_mm_arc();
    let confpath = unsafe { QTUM_CONF_PATH.as_ref().expect("Qtum config is not set yet") };
    let conf = json!({
        "coin":ticker,
        "decimals": 8,
        "required_confirmations":0,
        "pubtype":120,
        "p2shtype":110,
        "wiftype":128,
        "segwit":true,
        "mm2":1,
        "mature_confirmations":500,
        "network":"regtest",
        "confpath": confpath,
        "dust": 72800,
    });
    let req = json!({
        "method": "enable",
        "swap_contract_address": format!("{:#02x}", swap_contract_address),
    });
    let coin = block_on(qrc20_coin_from_conf_and_request(
        &ctx,
        ticker,
        platform,
        &conf,
        &req,
        priv_key,
        contract_address,
    ))
    .unwrap();

    import_address(&coin);
    (ctx, coin)
}

/// Generate random privkey, create a QRC20 coin and fill it's address with the specified balance.
fn generate_qrc20_coin_with_random_privkey(
    ticker: &str,
    qtum_balance: BigDecimal,
    qrc20_balance: BigDecimal,
) -> (MmArc, Qrc20Coin, [u8; 32]) {
    let priv_key = SecretKey::new(&mut rand6::thread_rng());
    let (ctx, coin) = qrc20_coin_from_privkey(ticker, priv_key.as_ref());

    let timeout = 30; // timeout if test takes more than 30 seconds to run
    let my_address = coin.my_address().expect("!my_address");
    fill_address(&coin, &my_address, qtum_balance, timeout);
    fill_qrc20_address(&coin, qrc20_balance, timeout);
    (ctx, coin, *priv_key.as_ref())
}

fn generate_qtum_coin_with_random_privkey(
    ticker: &str,
    balance: BigDecimal,
    txfee: Option<u64>,
) -> (MmArc, QtumCoin, [u8; 32]) {
    let confpath = unsafe { QTUM_CONF_PATH.as_ref().expect("Qtum config is not set yet") };
    let conf = json!({
        "coin":ticker,
        "decimals":8,
        "required_confirmations":0,
        "pubtype":120,
        "p2shtype": 110,
        "wiftype":128,
        "segwit":true,
        "txfee": txfee,
        "txfee_volatility_percent":0.1,
        "mm2":1,
        "mature_confirmations":500,
        "network":"regtest",
        "confpath": confpath,
        "dust": 72800,
    });
    let req = json!({"method": "enable"});
    let priv_key = SecretKey::new(&mut rand6::thread_rng());
    let ctx = MmCtxBuilder::new().into_mm_arc();
    let coin = block_on(qtum_coin_from_conf_and_request(
        &ctx,
        "QTUM",
        &conf,
        &req,
        priv_key.as_ref(),
    ))
    .unwrap();

    let timeout = 30; // timeout if test takes more than 30 seconds to run
    let my_address = coin.my_address().expect("!my_address");
    fill_address(&coin, &my_address, balance, timeout);
    (ctx, coin, *priv_key.as_ref())
}

fn generate_segwit_qtum_coin_with_random_privkey(
    ticker: &str,
    balance: BigDecimal,
    txfee: Option<u64>,
) -> (MmArc, QtumCoin, [u8; 32]) {
    let confpath = unsafe { QTUM_CONF_PATH.as_ref().expect("Qtum config is not set yet") };
    let conf = json!({
        "coin":ticker,
        "decimals":8,
        "required_confirmations":0,
        "pubtype":120,
        "p2shtype": 110,
        "wiftype":128,
        "segwit":true,
        "txfee": txfee,
        "txfee_volatility_percent":0.1,
        "mm2":1,
        "mature_confirmations":500,
        "network":"regtest",
        "confpath": confpath,
        "dust": 72800,
        "bech32_hrp":"qcrt",
        "address_format": {
            "format": "segwit",
        },
    });
    let req = json!({"method": "enable"});
    let priv_key = SecretKey::new(&mut rand6::thread_rng());
    let ctx = MmCtxBuilder::new().into_mm_arc();
    let coin = block_on(qtum_coin_from_conf_and_request(
        &ctx,
        "QTUM",
        &conf,
        &req,
        priv_key.as_ref(),
    ))
    .unwrap();

    let timeout = 30; // timeout if test takes more than 30 seconds to run
    let my_address = coin.my_address().expect("!my_address");
    fill_address(&coin, &my_address, balance, timeout);
    (ctx, coin, *priv_key.as_ref())
}

/// Get only one address assigned the specified label.
fn get_address_by_label<T>(coin: T, label: &str) -> String
where
    T: AsRef<UtxoCoinFields>,
{
    let native = match coin.as_ref().rpc_client {
        UtxoRpcClientEnum::Native(ref native) => native,
        UtxoRpcClientEnum::Electrum(_) => panic!("NativeClient expected"),
    };
    let mut addresses = native
        .get_addresses_by_label(label)
        .wait()
        .expect("!getaddressesbylabel")
        .into_iter();
    match addresses.next() {
        Some((addr, _purpose)) if addresses.next().is_none() => addr,
        Some(_) => panic!("Expected only one address by {:?}", label),
        None => panic!("Expected one address by {:?}", label),
    }
}

fn fill_qrc20_address(coin: &Qrc20Coin, amount: BigDecimal, timeout: u64) {
    // prevent concurrent fill since daemon RPC returns errors if send_to_address
    // is called concurrently (insufficient funds) and it also may return other errors
    // if previous transaction is not confirmed yet
    let _lock = COINS_LOCK.lock().unwrap();
    let timeout = now_ms() / 1000 + timeout;
    let client = match coin.as_ref().rpc_client {
        UtxoRpcClientEnum::Native(ref client) => client,
        UtxoRpcClientEnum::Electrum(_) => panic!("Expected NativeClient"),
    };

    let from_addr = get_address_by_label(coin, QTUM_ADDRESS_LABEL);
    let to_addr = coin.my_addr_as_contract_addr();
    let satoshis = sat_from_big_decimal(&amount, coin.as_ref().decimals).expect("!sat_from_big_decimal");

    let hash = client
        .transfer_tokens(
            &coin.contract_address,
            &from_addr,
            to_addr,
            satoshis.into(),
            coin.as_ref().decimals,
        )
        .wait()
        .expect("!transfer_tokens")
        .txid;

    let tx_bytes = client.get_transaction_bytes(hash).wait().unwrap();
    log!({ "{:02x}", tx_bytes });
    coin.wait_for_confirmations(&tx_bytes, 1, false, timeout, 1)
        .wait()
        .unwrap();
}

pub async fn enable_qrc20_native(mm: &MarketMakerIt, coin: &str) -> Json {
    let swap_contract_address =
        unsafe { QRC20_SWAP_CONTRACT_ADDRESS.expect("QRC20_SWAP_CONTRACT_ADDRESS must be set already") };

    let native = mm
        .rpc(json! ({
            "userpass": mm.userpass,
            "method": "enable",
            "coin": coin,
            "swap_contract_address": format!("{:#02x}", swap_contract_address),
            "mm2": 1,
        }))
        .await
        .unwrap();
    assert_eq!(native.0, StatusCode::OK, "'enable' failed: {}", native.1);
    json::from_str(&native.1).unwrap()
}

/// Wait for the `estimatesmartfee` returns no errors.
fn wait_for_estimate_smart_fee(timeout: u64) -> Result<(), String> {
    enum EstimateSmartFeeState {
        Idle,
        Ok,
        NotAvailable,
    }
    lazy_static! {
        static ref LOCK: Mutex<EstimateSmartFeeState> = Mutex::new(EstimateSmartFeeState::Idle);
    }

    let state = &mut *LOCK.lock().unwrap();
    match state {
        EstimateSmartFeeState::Ok => return Ok(()),
        EstimateSmartFeeState::NotAvailable => return ERR!("estimatesmartfee not available"),
        EstimateSmartFeeState::Idle => log!("Start wait_for_estimate_smart_fee"),
    }

    let priv_key = SecretKey::new(&mut rand6::thread_rng());
    let (_ctx, coin) = qrc20_coin_from_privkey("QICK", priv_key.as_ref());
    let timeout = now_ms() / 1000 + timeout;
    let client = match coin.as_ref().rpc_client {
        UtxoRpcClientEnum::Native(ref client) => client,
        UtxoRpcClientEnum::Electrum(_) => panic!("Expected NativeClient"),
    };
    while now_ms() / 1000 < timeout {
        if let Ok(res) = client.estimate_smart_fee(&None, 1).wait() {
            if res.errors.is_empty() {
                *state = EstimateSmartFeeState::Ok;
                return Ok(());
            }
        }
        thread::sleep(Duration::from_secs(1));
    }

    *state = EstimateSmartFeeState::NotAvailable;
    ERR!("Waited too long for estimate_smart_fee to work")
}

fn qrc20_coin_conf_item(ticker: &str) -> Json {
    let contract_address = unsafe {
        match ticker {
            "QICK" => QICK_TOKEN_ADDRESS.expect("QICK_TOKEN_ADDRESS must be set already"),
            "QORTY" => QORTY_TOKEN_ADDRESS.expect("QORTY_TOKEN_ADDRESS must be set already"),
            _ => panic!("Expected either QICK or QORTY ticker, found {}", ticker),
        }
    };
    let contract_address = format!("{:#02x}", contract_address);

    let confpath = unsafe { QTUM_CONF_PATH.as_ref().expect("Qtum config is not set yet") };
    json!({
        "coin":ticker,
        "required_confirmations":1,
        "pubtype":120,
        "p2shtype":110,
        "wiftype":128,
        "segwit":true,
        "mature_confirmations":500,
        "confpath":confpath,
        "network":"regtest",
        "protocol":{"type":"QRC20","protocol_data":{"platform":"QTUM","contract_address":contract_address}}})
}

fn trade_base_rel((base, rel): (&str, &str)) {
    /// Generate a wallet with the random private key and fill the wallet with Qtum (required by gas_fee) and specified in `ticker` coin.
    fn generate_and_fill_priv_key(ticker: &str) -> [u8; 32] {
        let timeout = 30; // timeout if test takes more than 30 seconds to run

        match ticker {
            "QTUM" => {
                //Segwit QTUM
                wait_for_estimate_smart_fee(timeout).expect("!wait_for_estimate_smart_fee");
                let (_ctx, _coin, priv_key) = generate_segwit_qtum_coin_with_random_privkey("QTUM", 10.into(), Some(0));

                priv_key
            },
            "QICK" | "QORTY" => {
                let priv_key = SecretKey::new(&mut rand6::thread_rng());
                let (_ctx, coin) = qrc20_coin_from_privkey(ticker, priv_key.as_ref());
                let my_address = coin.my_address().expect("!my_address");
                fill_address(&coin, &my_address, 10.into(), timeout);
                fill_qrc20_address(&coin, 10.into(), timeout);

                *priv_key.as_ref()
            },
            "MYCOIN" | "MYCOIN1" => {
                let priv_key = SecretKey::new(&mut rand6::thread_rng());
                let (_ctx, coin) = utxo_coin_from_privkey(ticker, priv_key.as_ref());
                let my_address = coin.my_address().expect("!my_address");
                fill_address(&coin, &my_address, 10.into(), timeout);
                // also fill the Qtum
                let (_ctx, coin) = qrc20_coin_from_privkey("QICK", priv_key.as_ref());
                let my_address = coin.my_address().expect("!my_address");
                fill_address(&coin, &my_address, 10.into(), timeout);

                *priv_key.as_ref()
            },
            _ => panic!("Expected either QICK or QORTY or MYCOIN or MYCOIN1, found {}", ticker),
        }
    }

    let bob_priv_key = generate_and_fill_priv_key(base);
    let alice_priv_key = generate_and_fill_priv_key(rel);

    let confpath = unsafe { QTUM_CONF_PATH.as_ref().expect("Qtum config is not set yet") };
    let coins = json! ([
        qrc20_coin_conf_item("QICK"),
        qrc20_coin_conf_item("QORTY"),
        {"coin":"MYCOIN","asset":"MYCOIN","required_confirmations":0,"txversion":4,"overwintered":1,"txfee":1000,"protocol":{"type":"UTXO"}},
        {"coin":"MYCOIN1","asset":"MYCOIN1","required_confirmations":0,"txversion":4,"overwintered":1,"txfee":1000,"protocol":{"type":"UTXO"}},
        {"coin":"QTUM","asset":"QTUM","required_confirmations":0,"decimals":8,"pubtype":120,"p2shtype":110,"wiftype":128,"segwit":true,"txfee":0,"txfee_volatility_percent":0.1,
        "mm2":1,"network":"regtest","confpath":confpath,"protocol":{"type":"UTXO"},"bech32_hrp":"qcrt","address_format":{"format":"segwit"}},
    ]);
    let mut mm_bob = MarketMakerIt::start(
        json! ({
            "gui": "nogui",
            "netid": 9000,
            "dht": "on",  // Enable DHT without delay.
            "passphrase": format!("0x{}", hex::encode(bob_priv_key)),
            "coins": coins,
            "rpc_password": "pass",
            "i_am_seed": true,
        }),
        "pass".to_string(),
        None,
    )
    .unwrap();
    let (_bob_dump_log, _bob_dump_dashboard) = mm_dump(&mm_bob.log_path);
    block_on(mm_bob.wait_for_log(22., |log| log.contains(">>>>>>>>> DEX stats "))).unwrap();

    let mut mm_alice = MarketMakerIt::start(
        json! ({
            "gui": "nogui",
            "netid": 9000,
            "dht": "on",  // Enable DHT without delay.
            "passphrase": format!("0x{}", hex::encode(alice_priv_key)),
            "coins": coins,
            "rpc_password": "pass",
            "seednodes": vec![format!("{}", mm_bob.ip)],
        }),
        "pass".to_string(),
        None,
    )
    .unwrap();
    let (_alice_dump_log, _alice_dump_dashboard) = mm_dump(&mm_alice.log_path);
    block_on(mm_alice.wait_for_log(22., |log| log.contains(">>>>>>>>> DEX stats "))).unwrap();

    log!([block_on(enable_qrc20_native(&mm_bob, "QICK"))]);
    log!([block_on(enable_qrc20_native(&mm_bob, "QORTY"))]);
    log!([block_on(enable_native(&mm_bob, "MYCOIN", &[]))]);
    log!([block_on(enable_native(&mm_bob, "MYCOIN1", &[]))]);
    log!([block_on(enable_native(&mm_bob, "QTUM", &[]))]);

    log!([block_on(enable_qrc20_native(&mm_alice, "QICK"))]);
    log!([block_on(enable_qrc20_native(&mm_alice, "QORTY"))]);
    log!([block_on(enable_native(&mm_alice, "MYCOIN", &[]))]);
    log!([block_on(enable_native(&mm_alice, "MYCOIN1", &[]))]);
    log!([block_on(enable_native(&mm_alice, "QTUM", &[]))]);
    let rc = block_on(mm_bob.rpc(json! ({
        "userpass": mm_bob.userpass,
        "method": "setprice",
        "base": base,
        "rel": rel,
        "price": 1,
        "volume": "3",
    })))
    .unwrap();
    assert!(rc.0.is_success(), "!setprice: {}", rc.1);

    thread::sleep(Duration::from_secs(12));

    log!("Issue alice " (base) "/" (rel) " buy request");
    let rc = block_on(mm_alice.rpc(json! ({
        "userpass": mm_alice.userpass,
        "method": "buy",
        "base": base,
        "rel": rel,
        "price": 1,
        "volume": "2",
    })))
    .unwrap();
    assert!(rc.0.is_success(), "!buy: {}", rc.1);
    let buy_json: Json = serde_json::from_str(&rc.1).unwrap();
    let uuid = buy_json["result"]["uuid"].as_str().unwrap().to_owned();

    // ensure the swaps are started
    block_on(mm_bob.wait_for_log(22., |log| {
        log.contains(&format!("Entering the maker_swap_loop {}/{}", base, rel))
    }))
    .unwrap();
    block_on(mm_alice.wait_for_log(22., |log| {
        log.contains(&format!("Entering the taker_swap_loop {}/{}", base, rel))
    }))
    .unwrap();

    // ensure the swaps are finished
    block_on(mm_bob.wait_for_log(600., |log| log.contains(&format!("[swap uuid={}] Finished", uuid)))).unwrap();
    block_on(mm_alice.wait_for_log(600., |log| log.contains(&format!("[swap uuid={}] Finished", uuid)))).unwrap();

    log!("Checking alice/taker status..");
    block_on(check_my_swap_status(
        &mm_alice,
        &uuid,
        &TAKER_SUCCESS_EVENTS,
        &TAKER_ERROR_EVENTS,
        "2".parse().unwrap(),
        "2".parse().unwrap(),
    ));

    log!("Checking bob/maker status..");
    block_on(check_my_swap_status(
        &mm_bob,
        &uuid,
        &MAKER_SUCCESS_EVENTS,
        &MAKER_ERROR_EVENTS,
        "2".parse().unwrap(),
        "2".parse().unwrap(),
    ));

    log!("Waiting 3 seconds for nodes to broadcast their swaps data..");
    thread::sleep(Duration::from_secs(3));

    log!("Checking alice status..");
    block_on(check_stats_swap_status(
        &mm_alice,
        &uuid,
        &MAKER_SUCCESS_EVENTS,
        &TAKER_SUCCESS_EVENTS,
    ));

    log!("Checking bob status..");
    block_on(check_stats_swap_status(
        &mm_bob,
        &uuid,
        &MAKER_SUCCESS_EVENTS,
        &TAKER_SUCCESS_EVENTS,
    ));

    log!("Checking alice recent swaps..");
    block_on(check_recent_swaps(&mm_alice, 1));
    log!("Checking bob recent swaps..");
    block_on(check_recent_swaps(&mm_bob, 1));

    block_on(mm_bob.stop()).unwrap();
    block_on(mm_alice.stop()).unwrap();
}

fn withdraw_and_send(mm: &MarketMakerIt, coin: &str, to: &str, amount: f64) {
    let withdraw = block_on(mm.rpc(json! ({
        "mmrpc": "2.0",
        "userpass": mm.userpass,
        "method": "withdraw",
        "params": {
            "coin": coin,
            "to": to,
            "amount": amount,
        },
        "id": 0,
    })))
    .unwrap();

    assert!(withdraw.0.is_success(), "!withdraw: {}", withdraw.1);
    let res: RpcSuccessResponse<TransactionDetails> =
        json::from_str(&withdraw.1).expect("Expected 'RpcSuccessResponse<TransactionDetails>'");
    let tx_details = res.result;

    log!("Balance Change: "[tx_details.my_balance_change]);

    assert_eq!(tx_details.to, vec![to.to_owned()]);
    assert!(BigDecimal::from(amount) + tx_details.my_balance_change < 0.into());

    let send = block_on(mm.rpc(json! ({
        "userpass": mm.userpass,
        "method": "send_raw_transaction",
        "coin": coin,
        "tx_hex": tx_details.tx_hex,
    })))
    .unwrap();
    assert!(send.0.is_success(), "!{} send: {}", coin, send.1);
    let send_json: Json = json::from_str(&send.1).unwrap();
    assert_eq!(tx_details.tx_hash, send_json["tx_hash"]);
}

#[test]
fn test_taker_spends_maker_payment() {
    let (_ctx, maker_coin, _priv_key) = generate_qrc20_coin_with_random_privkey("QICK", 20.into(), 10.into());
    let (_ctx, taker_coin, _priv_key) = generate_qrc20_coin_with_random_privkey("QICK", 20.into(), 1.into());
    let maker_old_balance = maker_coin
        .my_spendable_balance()
        .wait()
        .expect("Error on get maker balance");
    let taker_old_balance = taker_coin
        .my_spendable_balance()
        .wait()
        .expect("Error on get taker balance");
    assert_eq!(maker_old_balance, BigDecimal::from(10));
    assert_eq!(taker_old_balance, BigDecimal::from(1));

    let timelock = (now_ms() / 1000) as u32 - 200;
    let maker_pub = &*maker_coin.my_public_key();
    let taker_pub = &*taker_coin.my_public_key();
    let secret = &[1; 32];
    let secret_hash = &*dhash160(secret);
    let amount = BigDecimal::from(0.2);

    let payment = maker_coin
        .send_maker_payment(
            timelock,
            taker_pub,
            secret_hash,
            amount.clone(),
            &maker_coin.swap_contract_address(),
        )
        .wait()
        .unwrap();
    let payment_tx_hash = payment.tx_hash();
    let payment_tx_hex = payment.tx_hex();
    log!("Maker payment: "[payment_tx_hash]);

    let confirmations = 1;
    let requires_nota = false;
    let wait_until = (now_ms() / 1000) + 40; // timeout if test takes more than 40 seconds to run
    let check_every = 1;
    taker_coin
        .wait_for_confirmations(&payment_tx_hex, confirmations, requires_nota, wait_until, check_every)
        .wait()
        .unwrap();

    taker_coin
        .validate_maker_payment(
            &payment_tx_hex,
            timelock,
            maker_pub,
            secret_hash,
            amount.clone(),
            &taker_coin.swap_contract_address(),
        )
        .wait()
        .unwrap();

    let spend = taker_coin
        .send_taker_spends_maker_payment(
            &payment_tx_hex,
            timelock,
            maker_pub,
            secret,
            &taker_coin.swap_contract_address(),
        )
        .wait()
        .unwrap();
    let spend_tx_hash = spend.tx_hash();
    let spend_tx_hex = spend.tx_hex();
    log!("Taker spends tx: "[spend_tx_hash]);

    let wait_until = (now_ms() / 1000) + 40; // timeout if test takes more than 40 seconds to run
    taker_coin
        .wait_for_confirmations(&spend_tx_hex, confirmations, requires_nota, wait_until, check_every)
        .wait()
        .unwrap();

    let maker_balance = maker_coin
        .my_spendable_balance()
        .wait()
        .expect("Error on get maker balance");
    let taker_balance = taker_coin
        .my_spendable_balance()
        .wait()
        .expect("Error on get taker balance");
    assert_eq!(maker_old_balance - amount.clone(), maker_balance);
    assert_eq!(taker_old_balance + amount, taker_balance);
}

#[test]
fn test_maker_spends_taker_payment() {
    let (_ctx, maker_coin, _priv_key) = generate_qrc20_coin_with_random_privkey("QICK", 20.into(), 10.into());
    let (_ctx, taker_coin, _priv_key) = generate_qrc20_coin_with_random_privkey("QICK", 20.into(), 10.into());
    let maker_old_balance = maker_coin
        .my_spendable_balance()
        .wait()
        .expect("Error on get maker balance");
    let taker_old_balance = taker_coin
        .my_spendable_balance()
        .wait()
        .expect("Error on get taker balance");
    assert_eq!(maker_old_balance, BigDecimal::from(10));
    assert_eq!(taker_old_balance, BigDecimal::from(10));

    let timelock = (now_ms() / 1000) as u32 - 200;
    let maker_pub = &*maker_coin.my_public_key();
    let taker_pub = &*taker_coin.my_public_key();
    let secret = &[1; 32];
    let secret_hash = &*dhash160(secret);
    let amount = BigDecimal::from(0.2);

    let payment = taker_coin
        .send_taker_payment(
            timelock,
            maker_pub,
            secret_hash,
            amount.clone(),
            &taker_coin.swap_contract_address(),
        )
        .wait()
        .unwrap();
    let payment_tx_hash = payment.tx_hash();
    let payment_tx_hex = payment.tx_hex();
    log!("Taker payment: "[payment_tx_hash]);

    let confirmations = 1;
    let requires_nota = false;
    let wait_until = (now_ms() / 1000) + 40; // timeout if test takes more than 40 seconds to run
    let check_every = 1;
    maker_coin
        .wait_for_confirmations(&payment_tx_hex, confirmations, requires_nota, wait_until, check_every)
        .wait()
        .unwrap();

    maker_coin
        .validate_taker_payment(
            &payment_tx_hex,
            timelock,
            taker_pub,
            secret_hash,
            amount.clone(),
            &maker_coin.swap_contract_address(),
        )
        .wait()
        .unwrap();

    let spend = maker_coin
        .send_maker_spends_taker_payment(
            &payment_tx_hex,
            timelock,
            taker_pub,
            secret,
            &maker_coin.swap_contract_address(),
        )
        .wait()
        .unwrap();
    let spend_tx_hash = spend.tx_hash();
    let spend_tx_hex = spend.tx_hex();
    log!("Maker spends tx: "[spend_tx_hash]);

    let wait_until = (now_ms() / 1000) + 40; // timeout if test takes more than 40 seconds to run
    maker_coin
        .wait_for_confirmations(&spend_tx_hex, confirmations, requires_nota, wait_until, check_every)
        .wait()
        .unwrap();

    let maker_balance = maker_coin
        .my_spendable_balance()
        .wait()
        .expect("Error on get maker balance");
    let taker_balance = taker_coin
        .my_spendable_balance()
        .wait()
        .expect("Error on get taker balance");
    assert_eq!(maker_old_balance + amount.clone(), maker_balance);
    assert_eq!(taker_old_balance - amount, taker_balance);
}

#[test]
fn test_maker_refunds_payment() {
    let (_ctx, coin, _priv_key) = generate_qrc20_coin_with_random_privkey("QICK", 20.into(), 10.into());
    let expected_balance = coin.my_spendable_balance().wait().unwrap();
    assert_eq!(expected_balance, BigDecimal::from(10));

    let timelock = (now_ms() / 1000) as u32 - 200;
    let taker_pub = hex::decode("022b00078841f37b5d30a6a1defb82b3af4d4e2d24dd4204d41f0c9ce1e875de1a").unwrap();
    let secret_hash = &[1; 20];
    let amount = BigDecimal::from_str("0.2").unwrap();

    let payment = coin
        .send_maker_payment(
            timelock,
            &taker_pub,
            secret_hash,
            amount.clone(),
            &coin.swap_contract_address(),
        )
        .wait()
        .unwrap();
    let payment_tx_hash = payment.tx_hash();
    let payment_tx_hex = payment.tx_hex();
    log!("Maker payment: "[payment_tx_hash]);

    let confirmations = 1;
    let requires_nota = false;
    let wait_until = (now_ms() / 1000) + 40; // timeout if test takes more than 40 seconds to run
    let check_every = 1;
    coin.wait_for_confirmations(&payment_tx_hex, confirmations, requires_nota, wait_until, check_every)
        .wait()
        .unwrap();

    let balance_after_payment = coin.my_spendable_balance().wait().unwrap();
    assert_eq!(expected_balance.clone() - amount, balance_after_payment);

    let refund = coin
        .send_maker_refunds_payment(
            &payment_tx_hex,
            timelock,
            &taker_pub,
            secret_hash,
            &coin.swap_contract_address(),
        )
        .wait()
        .unwrap();
    let refund_tx_hash = refund.tx_hash();
    let refund_tx_hex = refund.tx_hex();
    log!("Maker refunds payment: "[refund_tx_hash]);

    let wait_until = (now_ms() / 1000) + 40; // timeout if test takes more than 40 seconds to run
    coin.wait_for_confirmations(&refund_tx_hex, confirmations, requires_nota, wait_until, check_every)
        .wait()
        .unwrap();

    let balance_after_refund = coin.my_spendable_balance().wait().unwrap();
    assert_eq!(expected_balance, balance_after_refund);
}

#[test]
fn test_taker_refunds_payment() {
    let (_ctx, coin, _priv_key) = generate_qrc20_coin_with_random_privkey("QICK", 20.into(), 10.into());
    let expected_balance = coin.my_spendable_balance().wait().unwrap();
    assert_eq!(expected_balance, BigDecimal::from(10));

    let timelock = (now_ms() / 1000) as u32 - 200;
    let maker_pub = hex::decode("022b00078841f37b5d30a6a1defb82b3af4d4e2d24dd4204d41f0c9ce1e875de1a").unwrap();
    let secret_hash = &[1; 20];
    let amount = BigDecimal::from_str("0.2").unwrap();

    let payment = coin
        .send_taker_payment(
            timelock,
            &maker_pub,
            secret_hash,
            amount.clone(),
            &coin.swap_contract_address(),
        )
        .wait()
        .unwrap();
    let payment_tx_hash = payment.tx_hash();
    let payment_tx_hex = payment.tx_hex();
    log!("Taker payment: "[payment_tx_hash]);

    let confirmations = 1;
    let requires_nota = false;
    let wait_until = (now_ms() / 1000) + 40; // timeout if test takes more than 40 seconds to run
    let check_every = 1;
    coin.wait_for_confirmations(&payment_tx_hex, confirmations, requires_nota, wait_until, check_every)
        .wait()
        .unwrap();

    let balance_after_payment = coin.my_spendable_balance().wait().unwrap();
    assert_eq!(expected_balance.clone() - amount, balance_after_payment);

    let refund = coin
        .send_taker_refunds_payment(
            &payment_tx_hex,
            timelock,
            &maker_pub,
            secret_hash,
            &coin.swap_contract_address(),
        )
        .wait()
        .unwrap();
    let refund_tx_hash = refund.tx_hash();
    let refund_tx_hex = refund.tx_hex();
    log!("Taker refunds payment: "[refund_tx_hash]);

    let wait_until = (now_ms() / 1000) + 40; // timeout if test takes more than 40 seconds to run
    coin.wait_for_confirmations(&refund_tx_hex, confirmations, requires_nota, wait_until, check_every)
        .wait()
        .unwrap();

    let balance_after_refund = coin.my_spendable_balance().wait().unwrap();
    assert_eq!(expected_balance, balance_after_refund);
}

#[test]
fn test_check_if_my_payment_sent() {
    let (_ctx, coin, _priv_key) = generate_qrc20_coin_with_random_privkey("QICK", 20.into(), 10.into());
    let timelock = (now_ms() / 1000) as u32 - 200;
    let taker_pub = hex::decode("022b00078841f37b5d30a6a1defb82b3af4d4e2d24dd4204d41f0c9ce1e875de1a").unwrap();
    let secret_hash = &[1; 20];
    let amount = BigDecimal::from_str("0.2").unwrap();

    let payment = coin
        .send_maker_payment(timelock, &taker_pub, secret_hash, amount, &coin.swap_contract_address())
        .wait()
        .unwrap();
    let payment_tx_hash = payment.tx_hash();
    let payment_tx_hex = payment.tx_hex();
    log!("Maker payment: "[payment_tx_hash]);

    let confirmations = 2;
    let requires_nota = false;
    let wait_until = (now_ms() / 1000) + 40; // timeout if test takes more than 40 seconds to run
    let check_every = 1;
    coin.wait_for_confirmations(&payment_tx_hex, confirmations, requires_nota, wait_until, check_every)
        .wait()
        .unwrap();

    let search_from_block = coin.current_block().wait().expect("!current_block") - 10;
    let found = coin
        .check_if_my_payment_sent(
            timelock,
            &taker_pub,
            secret_hash,
            search_from_block,
            &coin.swap_contract_address(),
        )
        .wait()
        .unwrap();
    assert_eq!(found, Some(payment));
}

#[test]
fn test_search_for_swap_tx_spend_taker_spent() {
    let (_ctx, maker_coin, _priv_key) = generate_qrc20_coin_with_random_privkey("QICK", 20.into(), 10.into());
    let (_ctx, taker_coin, _priv_key) = generate_qrc20_coin_with_random_privkey("QICK", 20.into(), 1.into());
    let search_from_block = maker_coin.current_block().wait().expect("!current_block");

    let timelock = (now_ms() / 1000) as u32 - 200;
    let maker_pub = &*maker_coin.my_public_key();
    let taker_pub = &*taker_coin.my_public_key();
    let secret = &[1; 32];
    let secret_hash = &*dhash160(secret);
    let amount = BigDecimal::from(0.2);

    let payment = maker_coin
        .send_maker_payment(
            timelock,
            taker_pub,
            secret_hash,
            amount,
            &maker_coin.swap_contract_address(),
        )
        .wait()
        .unwrap();
    let payment_tx_hash = payment.tx_hash();
    let payment_tx_hex = payment.tx_hex();
    log!("Maker payment: "[payment_tx_hash]);

    let confirmations = 1;
    let requires_nota = false;
    let wait_until = (now_ms() / 1000) + 40; // timeout if test takes more than 40 seconds to run
    let check_every = 1;
    taker_coin
        .wait_for_confirmations(&payment_tx_hex, confirmations, requires_nota, wait_until, check_every)
        .wait()
        .unwrap();

    let spend = taker_coin
        .send_taker_spends_maker_payment(
            &payment_tx_hex,
            timelock,
            maker_pub,
            secret,
            &taker_coin.swap_contract_address(),
        )
        .wait()
        .unwrap();
    let spend_tx_hash = spend.tx_hash();
    let spend_tx_hex = spend.tx_hex();
    log!("Taker spends tx: "[spend_tx_hash]);

    let wait_until = (now_ms() / 1000) + 40; // timeout if test takes more than 40 seconds to run
    taker_coin
        .wait_for_confirmations(&spend_tx_hex, confirmations, requires_nota, wait_until, check_every)
        .wait()
        .unwrap();

    let actual = maker_coin.search_for_swap_tx_spend_my(
        timelock,
        taker_pub,
        secret_hash,
        &payment_tx_hex,
        search_from_block,
        &maker_coin.swap_contract_address(),
    );
    let expected = Ok(Some(FoundSwapTxSpend::Spent(spend)));
    assert_eq!(actual, expected);
}

#[test]
fn test_search_for_swap_tx_spend_maker_refunded() {
    let (_ctx, maker_coin, _priv_key) = generate_qrc20_coin_with_random_privkey("QICK", 20.into(), 10.into());
    let search_from_block = maker_coin.current_block().wait().expect("!current_block");

    let timelock = (now_ms() / 1000) as u32 - 200;
    let taker_pub = hex::decode("022b00078841f37b5d30a6a1defb82b3af4d4e2d24dd4204d41f0c9ce1e875de1a").unwrap();
    let secret = &[1; 32];
    let secret_hash = &*dhash160(secret);
    let amount = BigDecimal::from(0.2);

    let payment = maker_coin
        .send_maker_payment(
            timelock,
            &taker_pub,
            secret_hash,
            amount,
            &maker_coin.swap_contract_address(),
        )
        .wait()
        .unwrap();
    let payment_tx_hash = payment.tx_hash();
    let payment_tx_hex = payment.tx_hex();
    log!("Maker payment: "[payment_tx_hash]);

    let confirmations = 1;
    let requires_nota = false;
    let wait_until = (now_ms() / 1000) + 40; // timeout if test takes more than 40 seconds to run
    let check_every = 1;
    maker_coin
        .wait_for_confirmations(&payment_tx_hex, confirmations, requires_nota, wait_until, check_every)
        .wait()
        .unwrap();

    let refund = maker_coin
        .send_maker_refunds_payment(
            &payment_tx_hex,
            timelock,
            &taker_pub,
            secret_hash,
            &maker_coin.swap_contract_address(),
        )
        .wait()
        .unwrap();
    let refund_tx_hash = refund.tx_hash();
    let refund_tx_hex = refund.tx_hex();
    log!("Maker refunds tx: "[refund_tx_hash]);

    let wait_until = (now_ms() / 1000) + 40; // timeout if test takes more than 40 seconds to run
    maker_coin
        .wait_for_confirmations(&refund_tx_hex, confirmations, requires_nota, wait_until, check_every)
        .wait()
        .unwrap();

    let actual = maker_coin.search_for_swap_tx_spend_my(
        timelock,
        &taker_pub,
        secret_hash,
        &payment_tx_hex,
        search_from_block,
        &maker_coin.swap_contract_address(),
    );
    let expected = Ok(Some(FoundSwapTxSpend::Refunded(refund)));
    assert_eq!(actual, expected);
}

#[test]
fn test_search_for_swap_tx_spend_not_spent() {
    let (_ctx, maker_coin, _priv_key) = generate_qrc20_coin_with_random_privkey("QICK", 20.into(), 10.into());
    let search_from_block = maker_coin.current_block().wait().expect("!current_block");

    let timelock = (now_ms() / 1000) as u32 - 200;
    let taker_pub = hex::decode("022b00078841f37b5d30a6a1defb82b3af4d4e2d24dd4204d41f0c9ce1e875de1a").unwrap();
    let secret = &[1; 32];
    let secret_hash = &*dhash160(secret);
    let amount = BigDecimal::from(0.2);

    let payment = maker_coin
        .send_maker_payment(
            timelock,
            &taker_pub,
            secret_hash,
            amount,
            &maker_coin.swap_contract_address(),
        )
        .wait()
        .unwrap();
    let payment_tx_hash = payment.tx_hash();
    let payment_tx_hex = payment.tx_hex();
    log!("Maker payment: "[payment_tx_hash]);

    let confirmations = 1;
    let requires_nota = false;
    let wait_until = (now_ms() / 1000) + 40; // timeout if test takes more than 40 seconds to run
    let check_every = 1;
    maker_coin
        .wait_for_confirmations(&payment_tx_hex, confirmations, requires_nota, wait_until, check_every)
        .wait()
        .unwrap();

    let actual = maker_coin.search_for_swap_tx_spend_my(
        timelock,
        &taker_pub,
        secret_hash,
        &payment_tx_hex,
        search_from_block,
        &maker_coin.swap_contract_address(),
    );
    // maker payment hasn't been spent or refunded yet
    assert_eq!(actual, Ok(None));
}

#[test]
fn test_wait_for_tx_spend() {
    let (_ctx, maker_coin, _priv_key) = generate_qrc20_coin_with_random_privkey("QICK", 20.into(), 10.into());
    let (_ctx, taker_coin, _priv_key) = generate_qrc20_coin_with_random_privkey("QICK", 20.into(), 1.into());
    let from_block = maker_coin.current_block().wait().expect("!current_block");

    let timelock = (now_ms() / 1000) as u32 - 200;
    let maker_pub = &*maker_coin.my_public_key();
    let taker_pub = &*taker_coin.my_public_key();
    let secret = &[1; 32];
    let secret_hash = &*dhash160(secret);
    let amount = BigDecimal::from(0.2);

    let payment = maker_coin
        .send_maker_payment(
            timelock,
            taker_pub,
            secret_hash,
            amount,
            &maker_coin.swap_contract_address(),
        )
        .wait()
        .unwrap();
    let payment_tx_hash = payment.tx_hash();
    let payment_tx_hex = payment.tx_hex();
    log!("Maker payment: "[payment_tx_hash]);

    let confirmations = 1;
    let requires_nota = false;
    let wait_until = (now_ms() / 1000) + 40; // timeout if test takes more than 40 seconds to run
    let check_every = 1;
    taker_coin
        .wait_for_confirmations(&payment_tx_hex, confirmations, requires_nota, wait_until, check_every)
        .wait()
        .unwrap();

    // first try to check if the wait_for_tx_spend() returns an error correctly
    let wait_until = (now_ms() / 1000) + 5;
    let err = maker_coin
        .wait_for_tx_spend(
            &payment_tx_hex,
            wait_until,
            from_block,
            &maker_coin.swap_contract_address(),
        )
        .wait()
        .expect_err("Expected 'Waited too long' error");
    log!("error: "[err]);
    assert!(err.contains("Waited too long"));

    // also spends the maker payment and try to check if the wait_for_tx_spend() returns the correct tx
    static mut SPEND_TX: Option<TransactionEnum> = None;

    let maker_pub_c = maker_pub.to_vec();
    let payment_hex = payment_tx_hex.clone();
    thread::spawn(move || {
        thread::sleep(Duration::from_secs(5));

        let spend = taker_coin
            .send_taker_spends_maker_payment(
                &payment_hex,
                timelock,
                &maker_pub_c,
                secret,
                &taker_coin.swap_contract_address(),
            )
            .wait()
            .unwrap();
        unsafe { SPEND_TX = Some(spend) }
    });

    let wait_until = (now_ms() / 1000) + 120;
    let found = maker_coin
        .wait_for_tx_spend(
            &payment_tx_hex,
            wait_until,
            from_block,
            &maker_coin.swap_contract_address(),
        )
        .wait()
        .unwrap();

    unsafe { assert_eq!(Some(found), SPEND_TX) }
}

#[test]
fn test_check_balance_on_order_post_base_coin_locked() {
    let bob_priv_key = SecretKey::new(&mut rand6::thread_rng());
    let alice_priv_key = SecretKey::new(&mut rand6::thread_rng());
    let timeout = 30; // timeout if test takes more than 80 seconds to run

    // fill the Bob address by 0.05 Qtum
    let (_ctx, coin) = qrc20_coin_from_privkey("QICK", bob_priv_key.as_ref());
    let my_address = coin.my_address().expect("!my_address");
    fill_address(&coin, &my_address, 0.05.into(), timeout);
    // fill the Bob address by 10 MYCOIN
    let (_ctx, coin) = utxo_coin_from_privkey("MYCOIN", bob_priv_key.as_ref());
    let my_address = coin.my_address().expect("!my_address");
    fill_address(&coin, &my_address, 10.into(), timeout);

    // fill the Alice address by 10 Qtum and 10 QICK
    let (_ctx, coin) = qrc20_coin_from_privkey("QICK", alice_priv_key.as_ref());
    let my_address = coin.my_address().expect("!my_address");
    fill_address(&coin, &my_address, 10.into(), timeout);
    fill_qrc20_address(&coin, 10.into(), timeout);
    // fill the Alice address by 10 MYCOIN
    let (_ctx, coin) = utxo_coin_from_privkey("MYCOIN", alice_priv_key.as_ref());
    let my_address = coin.my_address().expect("!my_address");
    fill_address(&coin, &my_address, 10.into(), timeout);

    let confpath = unsafe { QTUM_CONF_PATH.as_ref().expect("Qtum config is not set yet") };
    let qick_contract_address = format!("{:#02x}", unsafe { QICK_TOKEN_ADDRESS.expect("!QICK_TOKEN_ADDRESS") });
    let coins = json!([
        {"coin":"MYCOIN","asset":"MYCOIN","required_confirmations":0,"txversion":4,"overwintered":1,"txfee":1000,"protocol":{"type":"UTXO"}},
        {"coin":"QICK","required_confirmations":1,"pubtype": 120,"p2shtype": 50,"wiftype": 128,"segwit": true,"mm2": 1,"mature_confirmations": 500,"confpath": confpath,"network":"regtest",
         "protocol":{"type":"QRC20","protocol_data":{"platform":"QTUM","contract_address":qick_contract_address}}},
    ]);

    let mut mm_bob = MarketMakerIt::start(
        json! ({
            "gui": "nogui",
            "netid": 9998,
            "myipaddr": env::var ("BOB_TRADE_IP") .ok(),
            "rpcip": env::var ("BOB_TRADE_IP") .ok(),
            "canbind": env::var ("BOB_TRADE_PORT") .ok().map(|s| s.parse::<i64>().unwrap()),
            "passphrase": format!("0x{}", hex::encode(bob_priv_key.as_ref())),
            "coins": coins,
            "i_am_seed": true,
            "rpc_password": "pass",
        }),
        "pass".into(),
        None,
    )
    .unwrap();
    let (_dump_log, _dump_dashboard) = mm_dump(&mm_bob.log_path);
    log!({"Log path: {}", mm_bob.log_path.display()});
    block_on(mm_bob.wait_for_log(22., |log| log.contains(">>>>>>>>> DEX stats "))).unwrap();
    block_on(enable_native(&mm_bob, "MYCOIN", &[]));
    block_on(enable_qrc20_native(&mm_bob, "QICK"));

    // start alice
    let mut mm_alice = MarketMakerIt::start(
        json! ({
            "gui": "nogui",
            "netid": 9998,
            "myipaddr": env::var ("BOB_TRADE_IP") .ok(),
            "rpcip": env::var ("BOB_TRADE_IP") .ok(),
            "canbind": env::var ("BOB_TRADE_PORT") .ok().map(|s| s.parse::<i64>().unwrap()),
            "passphrase": format!("0x{}", hex::encode(alice_priv_key.as_ref())),
            "coins": coins,
            "seednodes": [fomat!((mm_bob.ip))],
            "rpc_password": "pass",
        }),
        "pass".into(),
        None,
    )
    .unwrap();
    let (_dump_log, _dump_dashboard) = mm_dump(&mm_alice.log_path);
    log!({"Log path: {}", mm_alice.log_path.display()});
    block_on(mm_alice.wait_for_log(22., |log| log.contains(">>>>>>>>> DEX stats "))).unwrap();
    block_on(enable_native(&mm_alice, "MYCOIN", &[]));
    block_on(enable_qrc20_native(&mm_alice, "QICK"));

    let rc = block_on(mm_alice.rpc(json! ({
        "userpass": mm_alice.userpass,
        "method": "setprice",
        "base": "QICK",
        "rel": "MYCOIN",
        "price": 1,
        "volume": 1,
    })))
    .unwrap();
    assert!(rc.0.is_success(), "!sell: {}", rc.1);

    log!("Give Bob 2 seconds to import Alice order");
    thread::sleep(Duration::from_secs(2));

    // Buy QICK and thus lock ~ 0.05 Qtum
    let rc = block_on(mm_bob.rpc(json! ({
        "userpass": mm_alice.userpass,
        "method": "buy",
        "base": "QICK",
        "rel": "MYCOIN",
        "price": 1,
        "volume": 1,
    })))
    .unwrap();
    assert!(rc.0.is_success(), "!buy: {}", rc.1);

    log!("Give swaps some time to start");
    thread::sleep(Duration::from_secs(4));

    // QRC20 balance is sufficient, but most of the balance is locked
    let rc = block_on(mm_bob.rpc(json! ({
        "userpass": mm_bob.userpass,
        "method": "sell",
        "base": "MYCOIN",
        "rel": "QICK",
        "price": 1,
        "volume": 1,
    })))
    .unwrap();
    assert!(!rc.0.is_success(), "!sell success but should be error: {}", rc.1);
}

/// Test the following statements:
/// * `max_taker_vol` returns an expected volume. This expected volume is calculated according to the instructions described in the comments to the [`lp_swap::taker_swap::max_taker_vol`] function;
/// * If we issue a `sell` request it never fails;
/// * Our balance is sufficient to send `TakerFee` and `TakerPayment` with the expected volume;
/// * Zero left on QTUM balance.
///
/// Please note this function should be called before the Qtum balance is filled.
fn test_get_max_taker_vol_and_trade_with_dynamic_trade_fee(coin: QtumCoin, priv_key: &[u8]) {
    let confpath = unsafe { QTUM_CONF_PATH.as_ref().expect("Qtum config is not set yet") };
    let coins = json! ([
        {"coin":"MYCOIN","asset":"MYCOIN","txversion":4,"overwintered":1,"txfee":1000,"protocol":{"type":"UTXO"}},
        {"coin":"QTUM","decimals":8,"pubtype":120,"p2shtype":110,"wiftype":128,"segwit":true,"txfee":0,"txfee_volatility_percent":0.1,
        "mm2":1,"mature_confirmations":500,"network":"regtest","confpath":confpath,"protocol":{"type":"UTXO"}},
    ]);
    let mut mm = MarketMakerIt::start(
        json! ({
            "gui": "nogui",
            "netid": 9000,
            "dht": "on",  // Enable DHT without delay.
            "passphrase": format!("0x{}", hex::encode(priv_key)),
            "coins": coins,
            "rpc_password": "pass",
            "i_am_seed": true,
        }),
        "pass".to_string(),
        None,
    )
    .unwrap();
    let (_alice_dump_log, _alice_dump_dashboard) = mm_dump(&mm.log_path);
    block_on(mm.wait_for_log(22., |log| log.contains(">>>>>>>>> DEX stats "))).unwrap();

    log!([block_on(enable_native(&mm, "MYCOIN", &[]))]);
    log!([block_on(enable_native(&mm, "QTUM", &[]))]);

    let qtum_balance = coin.my_spendable_balance().wait().expect("!my_balance");
    let qtum_dex_fee_threshold = MmNumber::from("0.000728");

    // - `max_possible = balance - locked_amount`, where `locked_amount = 0`
    // - `max_trade_fee = trade_fee(balance)`
    // Please note if we pass the exact value, the `get_sender_trade_fee` will fail with 'Not sufficient balance: Couldn't collect enough value from utxos'.
    // So we should deduct trade fee from the output.
    let max_trade_fee = coin
        .get_sender_trade_fee(
            TradePreimageValue::UpperBound(qtum_balance.clone()),
            FeeApproxStage::TradePreimage,
        )
        .wait()
        .expect("!get_sender_trade_fee");
    let max_trade_fee = max_trade_fee.amount.to_decimal();
    debug!("max_trade_fee: {}", max_trade_fee);

    // - `max_possible_2 = balance - locked_amount - max_trade_fee`, where `locked_amount = 0`
    let max_possible_2 = &qtum_balance - &max_trade_fee;
    // - `max_dex_fee = dex_fee(max_possible_2)`
    let max_dex_fee = dex_fee_amount(
        "QTUM",
        "MYCOIN",
        &MmNumber::from(max_possible_2),
        &qtum_dex_fee_threshold,
    );
    debug!("max_dex_fee: {:?}", max_dex_fee.to_fraction());

    // - `max_fee_to_send_taker_fee = fee_to_send_taker_fee(max_dex_fee)`
    // `taker_fee` is sent using general withdraw, and the fee get be obtained from withdraw result
    let max_fee_to_send_taker_fee = coin
        .get_fee_to_send_taker_fee(max_dex_fee.to_decimal(), FeeApproxStage::TradePreimage)
        .wait()
        .expect("!get_fee_to_send_taker_fee");
    let max_fee_to_send_taker_fee = max_fee_to_send_taker_fee.amount.to_decimal();
    debug!("max_fee_to_send_taker_fee: {}", max_fee_to_send_taker_fee);

    // and then calculate `min_max_val = balance - locked_amount - max_trade_fee - max_fee_to_send_taker_fee - dex_fee(max_val)` using `max_taker_vol_from_available()`
    // where `available = balance - locked_amount - max_trade_fee - max_fee_to_send_taker_fee`
    let available = &qtum_balance - &max_trade_fee - &max_fee_to_send_taker_fee;
    debug!("total_available: {}", available);
    let min_tx_amount = qtum_dex_fee_threshold.clone();
    let expected_max_taker_vol =
        max_taker_vol_from_available(MmNumber::from(available), "QTUM", "MYCOIN", &min_tx_amount)
            .expect("max_taker_vol_from_available");
    let real_dex_fee = dex_fee_amount("QTUM", "MYCOIN", &expected_max_taker_vol, &qtum_dex_fee_threshold);
    debug!("real_max_dex_fee: {:?}", real_dex_fee.to_fraction());

    // check if the actual max_taker_vol equals to the expected
    let rc = block_on(mm.rpc(json! ({
        "userpass": mm.userpass,
        "method": "max_taker_vol",
        "coin": "QTUM",
    })))
    .unwrap();
    assert!(rc.0.is_success(), "!max_taker_vol: {}", rc.1);
    let json: Json = json::from_str(&rc.1).unwrap();
    assert_eq!(
        json["result"],
        json::to_value(expected_max_taker_vol.to_fraction()).unwrap()
    );

    let rc = block_on(mm.rpc(json! ({
        "userpass": mm.userpass,
        "method": "sell",
        "base": "QTUM",
        "rel": "MYCOIN",
        "price": 1,
        "volume": expected_max_taker_vol.to_fraction(),
    })))
    .unwrap();
    assert!(rc.0.is_success(), "!sell: {}", rc.1);

    block_on(mm.stop()).unwrap();

    let timelock = (now_ms() / 1000) as u32 - 200;
    let secret_hash = &[0; 20];

    let dex_fee_amount = dex_fee_amount("QTUM", "MYCOIN", &expected_max_taker_vol, &qtum_dex_fee_threshold);
    let _taker_fee_tx = coin
        .send_taker_fee(&DEX_FEE_ADDR_RAW_PUBKEY, dex_fee_amount.to_decimal())
        .wait()
        .expect("!send_taker_fee");

    let _taker_payment_tx = coin
        .send_taker_payment(
            timelock,
            &DEX_FEE_ADDR_RAW_PUBKEY,
            secret_hash,
            expected_max_taker_vol.to_decimal(),
            &None,
        )
        .wait()
        .expect("!send_taker_payment");

    let my_balance = coin.my_spendable_balance().wait().expect("!my_balance");
    assert_eq!(
        my_balance,
        0.into(),
        "NOT AN ERROR, but it would be better if the balance remained zero"
    );
}

/// Generate the Qtum coin with a random balance and start the `test_get_max_taker_vol_and_trade_with_dynamic_trade_fee` test.
#[test]
fn test_max_taker_vol_dynamic_trade_fee() {
    wait_for_estimate_smart_fee(30).expect("!wait_for_estimate_smart_fee");
    // generate QTUM coin with the dynamic fee and fill the wallet by 2 Qtums
    let (_ctx, coin, priv_key) = generate_qtum_coin_with_random_privkey("QTUM", 2.into(), Some(0));
    let my_address = coin.my_address().expect("!my_address");
    let mut rng = rand6::thread_rng();
    let mut qtum_balance = BigDecimal::from(2);
    let mut qtum_balance_steps = "2".to_owned();
    for _ in 0..4 {
        let amount = rng.gen_range(100000, 10000000);
        let amount = big_decimal_from_sat(amount, 8);
        qtum_balance_steps = format!("{} + {}", qtum_balance_steps, amount);
        qtum_balance = &qtum_balance + &amount;
        fill_address(&coin, &my_address, amount, 30);
    }
    log!("QTUM balance "(qtum_balance)" = "(qtum_balance_steps));

    test_get_max_taker_vol_and_trade_with_dynamic_trade_fee(coin, &priv_key);
}

/// This is a special of a set of Qtum inputs where the `max_taker_payment` returns a volume such that
/// if the volume is passed into the `sell` request, the request will fail with `Not sufficient balance`.
/// This may be due to the `get_sender_trade_fee(balance)` called from `max_taker_payment` doesn't include the change output,
/// but the `get_sender_trade_fee(max_volume)` called from `sell` includes the change output.
/// To sum up, `get_sender_trade_fee(balance) < get_sender_trade_fee(max_volume)`, where `balance > max_volume`.
/// This test checks if the fee returned from `get_sender_trade_fee` should include the change output anyway.
#[test]
fn test_trade_preimage_fee_includes_change_output_anyway() {
    wait_for_estimate_smart_fee(30).expect("!wait_for_estimate_smart_fee");
    // generate QTUM coin with the dynamic fee and fill the wallet by 2 Qtums
    let (_ctx, coin, priv_key) = generate_qtum_coin_with_random_privkey("QTUM", 2.into(), Some(0));
    let my_address = coin.my_address().expect("!my_address");
    let mut qtum_balance = BigDecimal::from_str("2.2839365").expect("!BigDecimal::from_str");
    let amounts = vec!["0.09968324", "0.06979112", "0.09229586", "0.02216628"];
    for amount in amounts {
        let amount = BigDecimal::from_str(amount).expect("!BigDecimal::from_str");
        qtum_balance = &qtum_balance + &amount;
        fill_address(&coin, &my_address, amount, 30);
    }

    test_get_max_taker_vol_and_trade_with_dynamic_trade_fee(coin, &priv_key);
}

#[test]
fn test_trade_preimage_dynamic_fee_not_sufficient_balance() {
    wait_for_estimate_smart_fee(30).expect("!wait_for_estimate_smart_fee");
    // generate QTUM coin with the dynamic fee and fill the wallet by 0.5 Qtums
    let qtum_balance = MmNumber::from("0.5").to_decimal();
    let (_ctx, _coin, priv_key) = generate_qtum_coin_with_random_privkey("QTUM", qtum_balance.clone(), Some(0));

    let confpath = unsafe { QTUM_CONF_PATH.as_ref().expect("Qtum config is not set yet") };
    let coins = json! ([
        {"coin":"MYCOIN","asset":"MYCOIN","txversion":4,"overwintered":1,"txfee":1000,"protocol":{"type":"UTXO"}},
        {"coin":"QTUM","decimals":8,"pubtype":120,"p2shtype":110,"wiftype":128,"segwit":true,"txfee":0,"txfee_volatility_percent":0.1,
        "mm2":1,"mature_confirmations":500,"network":"regtest","confpath":confpath,"protocol":{"type":"UTXO"}},
    ]);
    let mut mm = MarketMakerIt::start(
        json! ({
            "gui": "nogui",
            "netid": 9000,
            "dht": "on",  // Enable DHT without delay.
            "passphrase": format!("0x{}", hex::encode(priv_key)),
            "coins": coins,
            "rpc_password": "pass",
            "i_am_seed": true,
        }),
        "pass".to_string(),
        None,
    )
    .unwrap();
    let (_alice_dump_log, _alice_dump_dashboard) = mm_dump(&mm.log_path);
    block_on(mm.wait_for_log(22., |log| log.contains(">>>>>>>>> DEX stats "))).unwrap();

    log!([block_on(enable_native(&mm, "MYCOIN", &[]))]);
    log!([block_on(enable_native(&mm, "QTUM", &[]))]);

    // txfee > 0, amount = 0.5 => required = txfee + amount > 0.5,
    // but balance = 0.5
    // This RPC call should fail because [`QtumCoin::get_sender_trade_fee`] will try to generate a dummy transaction due to the dynamic tx fee,
    // and this operation must fail with the [`TradePreimageError::NotSufficientBalance`].
    let rc = block_on(mm.rpc(json!({
        "userpass": mm.userpass,
        "mmrpc": "2.0",
        "method": "trade_preimage",
        "params": {
            "base": "QTUM",
            "rel": "MYCOIN",
            "swap_method": "setprice",
            "price": 1,
            "volume": qtum_balance,
        },
    })))
    .unwrap();
    assert!(!rc.0.is_success(), "trade_preimage success, but should fail: {}", rc.1);
    let actual: RpcErrorResponse<trade_preimage_error::NotSufficientBalance> = json::from_str(&rc.1).unwrap();
    assert_eq!(actual.error_type, "NotSufficientBalance");
    let data = actual.error_data.expect("Expected 'error_data'");
    assert_eq!(data.coin, "QTUM");
    assert_eq!(data.available, qtum_balance);
    assert!(data.required > qtum_balance);
}

/// If we try to deduct a transaction fee from `output = 0.00073`, the remaining value less than `dust = 0.000728`,
/// so we have to receive the `NotSufficientBalance` error.
#[test]
fn test_trade_preimage_deduct_fee_from_output_failed() {
    wait_for_estimate_smart_fee(30).expect("!wait_for_estimate_smart_fee");
    // generate QTUM coin with the dynamic fee and fill the wallet by 0.00073 Qtums (that is little greater than dust 0.000728)
    let qtum_balance = MmNumber::from("0.00073").to_decimal();
    let (_ctx, _coin, priv_key) = generate_qtum_coin_with_random_privkey("QTUM", qtum_balance.clone(), Some(0));

    let confpath = unsafe { QTUM_CONF_PATH.as_ref().expect("Qtum config is not set yet") };
    let coins = json! ([
        {"coin":"MYCOIN","asset":"MYCOIN","txversion":4,"overwintered":1,"txfee":1000,"protocol":{"type":"UTXO"}},
        {"coin":"QTUM","decimals":8,"pubtype":120,"p2shtype":110,"wiftype":128,"segwit":true,"txfee":0,"txfee_volatility_percent":0.1,
        "mm2":1,"mature_confirmations":500,"network":"regtest","confpath":confpath,"protocol":{"type":"UTXO"}},
    ]);
    let mut mm = MarketMakerIt::start(
        json! ({
            "gui": "nogui",
            "netid": 9000,
            "dht": "on",  // Enable DHT without delay.
            "passphrase": format!("0x{}", hex::encode(priv_key)),
            "coins": coins,
            "rpc_password": "pass",
            "i_am_seed": true,
        }),
        "pass".to_string(),
        None,
    )
    .unwrap();
    let (_alice_dump_log, _alice_dump_dashboard) = mm_dump(&mm.log_path);
    block_on(mm.wait_for_log(22., |log| log.contains(">>>>>>>>> DEX stats "))).unwrap();

    log!([block_on(enable_native(&mm, "MYCOIN", &[]))]);
    log!([block_on(enable_native(&mm, "QTUM", &[]))]);

    let rc = block_on(mm.rpc(json!({
        "userpass": mm.userpass,
        "mmrpc": "2.0",
        "method": "trade_preimage",
        "params": {
            "base": "QTUM",
            "rel": "MYCOIN",
            "swap_method": "setprice",
            "price": 1,
            "max": true,
        },
    })))
    .unwrap();
    assert!(!rc.0.is_success(), "trade_preimage success, but should fail: {}", rc.1);
    let actual: RpcErrorResponse<trade_preimage_error::NotSufficientBalance> = json::from_str(&rc.1).unwrap();
    assert_eq!(actual.error_type, "NotSufficientBalance");
    let trade_preimage_error::NotSufficientBalance {
        coin: actual_coin,
        available: actual_available,
        required: actual_required,
        ..
    } = actual.error_data.expect("Expected NotSufficientBalance error data");
    assert_eq!(actual_coin, "QTUM");
    assert_eq!(actual_available, qtum_balance);
    assert!(actual_required > qtum_balance);
}

#[test]
fn test_segwit_native_balance() {
    wait_for_estimate_smart_fee(30).expect("!wait_for_estimate_smart_fee");
    // generate QTUM coin with the dynamic fee and fill the wallet by 0.5 Qtums
    let (_ctx, _coin, priv_key) = generate_segwit_qtum_coin_with_random_privkey("QTUM", 0.5.into(), Some(0));

    let confpath = unsafe { QTUM_CONF_PATH.as_ref().expect("Qtum config is not set yet") };
    let coins = json! ([
        {"coin":"QTUM","decimals":8,"pubtype":120,"p2shtype":110,"wiftype":128,"segwit":true,"txfee":0,"txfee_volatility_percent":0.1,
        "mm2":1,"mature_confirmations":500,"network":"regtest","confpath":confpath,"protocol":{"type":"UTXO"},"bech32_hrp":"qcrt","address_format":{"format":"segwit"}},
    ]);
    let mut mm = MarketMakerIt::start(
        json! ({
            "gui": "nogui",
            "netid": 9000,
            "dht": "on",  // Enable DHT without delay.
            "passphrase": format!("0x{}", hex::encode(priv_key)),
            "coins": coins,
            "rpc_password": "pass",
            "i_am_seed": true,
        }),
        "pass".to_string(),
        None,
    )
    .unwrap();
    let (_alice_dump_log, _alice_dump_dashboard) = mm_dump(&mm.log_path);
    block_on(mm.wait_for_log(22., |log| log.contains(">>>>>>>>> DEX stats "))).unwrap();

    let enable_res = block_on(enable_native(&mm, "QTUM", &[]));
    let balance = enable_res["balance"].as_str().unwrap();
    assert_eq!(balance, "0.5");

    let my_balance = block_on(mm.rpc(json!({
        "userpass": mm.userpass,
        "method": "my_balance",
        "coin": "QTUM",
    })))
    .unwrap();
    let json: Json = json::from_str(&my_balance.1).unwrap();
    let my_balance = json["balance"].as_str().unwrap();
    assert_eq!(my_balance, "0.5");
    let my_unspendable_balance = json["unspendable_balance"].as_str().unwrap();
    assert_eq!(my_unspendable_balance, "0");
}

#[test]
fn test_withdraw_and_send_from_segwit() {
    wait_for_estimate_smart_fee(30).expect("!wait_for_estimate_smart_fee");
    // generate QTUM coin with the dynamic fee and fill the wallet by 0.5 Qtums
    let (_ctx, _coin, priv_key) = generate_segwit_qtum_coin_with_random_privkey("QTUM", 0.5.into(), Some(0));

    let confpath = unsafe { QTUM_CONF_PATH.as_ref().expect("Qtum config is not set yet") };
    let coins = json! ([
        {"coin":"QTUM","decimals":8,"pubtype":120,"p2shtype":110,"wiftype":128,"segwit":true,"txfee":0,"txfee_volatility_percent":0.1,
        "mm2":1,"mature_confirmations":500,"network":"regtest","confpath":confpath,"protocol":{"type":"UTXO"},"bech32_hrp":"qcrt","address_format":{"format":"segwit"}},
    ]);
    let mut mm = MarketMakerIt::start(
        json! ({
            "gui": "nogui",
            "netid": 9000,
            "dht": "on",  // Enable DHT without delay.
            "passphrase": format!("0x{}", hex::encode(priv_key)),
            "coins": coins,
            "rpc_password": "pass",
            "i_am_seed": true,
        }),
        "pass".to_string(),
        None,
    )
    .unwrap();
    let (_alice_dump_log, _alice_dump_dashboard) = mm_dump(&mm.log_path);
    block_on(mm.wait_for_log(22., |log| log.contains(">>>>>>>>> DEX stats "))).unwrap();

    log!([block_on(enable_native(&mm, "QTUM", &[]))]);

    // Send from Segwit Address to Segwit Address
    withdraw_and_send(&mm, "QTUM", "qcrt1q6pwxl4na4a363mgmrw8tjyppdcwuyfmat836dd", 0.2);

    // Send from Segwit Address to Legacy Address
    withdraw_and_send(&mm, "QTUM", "qVgbLqYPvKN5zH2eEJ6Jh8cjbUVx851yxV", 0.2);

    block_on(mm.stop()).unwrap();
}

#[test]
fn test_withdraw_and_send_legacy_to_segwit() {
    wait_for_estimate_smart_fee(30).expect("!wait_for_estimate_smart_fee");
    // generate QTUM coin with the dynamic fee and fill the wallet by 0.5 Qtums
    let (_ctx, _coin, priv_key) = generate_qtum_coin_with_random_privkey("QTUM", 0.5.into(), Some(0));

    let confpath = unsafe { QTUM_CONF_PATH.as_ref().expect("Qtum config is not set yet") };
    let coins = json! ([
        {"coin":"QTUM","decimals":8,"pubtype":120,"p2shtype":110,"wiftype":128,"segwit":true,"txfee":0,"txfee_volatility_percent":0.1,
        "mm2":1,"mature_confirmations":500,"network":"regtest","confpath":confpath,"protocol":{"type":"UTXO"},"bech32_hrp":"qcrt"},
    ]);
    let mut mm = MarketMakerIt::start(
        json! ({
            "gui": "nogui",
            "netid": 9000,
            "dht": "on",  // Enable DHT without delay.
            "passphrase": format!("0x{}", hex::encode(priv_key)),
            "coins": coins,
            "rpc_password": "pass",
            "i_am_seed": true,
        }),
        "pass".to_string(),
        None,
    )
    .unwrap();
    let (_alice_dump_log, _alice_dump_dashboard) = mm_dump(&mm.log_path);
    block_on(mm.wait_for_log(22., |log| log.contains(">>>>>>>>> DEX stats "))).unwrap();

    log!([block_on(enable_native(&mm, "QTUM", &[]))]);

    // Send from Legacy Address to Segwit Address
    withdraw_and_send(&mm, "QTUM", "qcrt1q6pwxl4na4a363mgmrw8tjyppdcwuyfmat836dd", 0.2);

    block_on(mm.stop()).unwrap();
}

#[test]
fn test_search_for_segwit_swap_tx_spend_native_was_refunded_maker() {
    wait_for_estimate_smart_fee(30).expect("!wait_for_estimate_smart_fee");
    let timeout = (now_ms() / 1000) + 120; // timeout if test takes more than 120 seconds to run
    let (_ctx, coin, _priv_key) = generate_segwit_qtum_coin_with_random_privkey("QTUM", 1000.into(), Some(0));

    let time_lock = (now_ms() / 1000) as u32 - 3600;
    let tx = coin
        .send_maker_payment(time_lock, &*coin.my_public_key(), &[0; 20], 1.into(), &None)
        .wait()
        .unwrap();

    coin.wait_for_confirmations(&tx.tx_hex(), 1, false, timeout, 1)
        .wait()
        .unwrap();

    let refund_tx = coin
        .send_maker_refunds_payment(&tx.tx_hex(), time_lock, &*coin.my_public_key(), &[0; 20], &None)
        .wait()
        .unwrap();

    coin.wait_for_confirmations(&refund_tx.tx_hex(), 1, false, timeout, 1)
        .wait()
        .unwrap();

    let found = coin
        .search_for_swap_tx_spend_my(time_lock, &*coin.my_public_key(), &[0; 20], &tx.tx_hex(), 0, &None)
        .unwrap()
        .unwrap();
    assert_eq!(FoundSwapTxSpend::Refunded(refund_tx), found);
}

#[test]
fn test_search_for_segwit_swap_tx_spend_native_was_refunded_taker() {
    wait_for_estimate_smart_fee(30).expect("!wait_for_estimate_smart_fee");
    let timeout = (now_ms() / 1000) + 120; // timeout if test takes more than 120 seconds to run
    let (_ctx, coin, _priv_key) = generate_segwit_qtum_coin_with_random_privkey("QTUM", 1000.into(), Some(0));

    let time_lock = (now_ms() / 1000) as u32 - 3600;
    let tx = coin
        .send_taker_payment(time_lock, &*coin.my_public_key(), &[0; 20], 1.into(), &None)
        .wait()
        .unwrap();

    coin.wait_for_confirmations(&tx.tx_hex(), 1, false, timeout, 1)
        .wait()
        .unwrap();

    let refund_tx = coin
        .send_taker_refunds_payment(&tx.tx_hex(), time_lock, &*coin.my_public_key(), &[0; 20], &None)
        .wait()
        .unwrap();

    coin.wait_for_confirmations(&refund_tx.tx_hex(), 1, false, timeout, 1)
        .wait()
        .unwrap();

    let found = coin
        .search_for_swap_tx_spend_my(time_lock, &*coin.my_public_key(), &[0; 20], &tx.tx_hex(), 0, &None)
        .unwrap()
        .unwrap();
    assert_eq!(FoundSwapTxSpend::Refunded(refund_tx), found);
}

pub async fn enable_native_segwit(mm: &MarketMakerIt, coin: &str) -> Json {
    let native = mm
        .rpc(json! ({
            "userpass": mm.userpass,
            "method": "enable",
            "coin": coin,
            "address_format": {
                "format": "segwit",
            },
            "mm2": 1,
        }))
        .await
        .unwrap();
    assert_eq!(native.0, StatusCode::OK, "'enable' failed: {}", native.1);
    json::from_str(&native.1).unwrap()
}

#[test]
#[ignore]
fn segwit_address_in_the_orderbook() {
    wait_for_estimate_smart_fee(30).expect("!wait_for_estimate_smart_fee");
    // generate QTUM coin with the dynamic fee and fill the wallet by 0.5 Qtums
    let (_ctx, coin, priv_key) = generate_qtum_coin_with_random_privkey("QTUM", 0.5.into(), Some(0));

    let confpath = unsafe { QTUM_CONF_PATH.as_ref().expect("Qtum config is not set yet") };
    let coins = json! ([
        {"coin":"QTUM","decimals":8,"pubtype":120,"p2shtype":110,"wiftype":128,"segwit":true,"txfee":0,"txfee_volatility_percent":0.1,
        "mm2":1,"mature_confirmations":500,"network":"regtest","confpath":confpath,"protocol":{"type":"UTXO"},"bech32_hrp":"qcrt"},
        {"coin":"MYCOIN","asset":"MYCOIN","txversion":4,"overwintered":1,"txfee":1000,"protocol":{"type":"UTXO"}},
    ]);
    let mut mm = MarketMakerIt::start(
        json! ({
            "gui": "nogui",
            "netid": 9000,
            "dht": "on",  // Enable DHT without delay.
            "passphrase": format!("0x{}", hex::encode(priv_key)),
            "coins": coins,
            "rpc_password": "pass",
            "i_am_seed": true,
        }),
        "pass".to_string(),
        None,
    )
    .unwrap();
    let (_alice_dump_log, _alice_dump_dashboard) = mm_dump(&mm.log_path);
    block_on(mm.wait_for_log(22., |log| log.contains(">>>>>>>>> DEX stats "))).unwrap();

    let enable_qtum_res = block_on(enable_native_segwit(&mm, "QTUM"));
    let enable_qtum_res: EnableElectrumResponse = json::from_value(enable_qtum_res).unwrap();
    let segwit_addr = enable_qtum_res.address;

    fill_address(&coin, &segwit_addr, 1000.into(), 30);

    log!([block_on(enable_native(&mm, "MYCOIN", &[]))]);

    let rc = block_on(mm.rpc(json! ({
        "userpass": mm.userpass,
        "method": "setprice",
        "base": "QTUM",
        "rel": "MYCOIN",
        "price": 1,
        "volume": "1",
    })))
    .unwrap();
    assert!(rc.0.is_success(), "!setprice: {}", rc.1);

    let orderbook = block_on(mm.rpc(json! ({
        "userpass": mm.userpass,
        "method": "orderbook",
        "base": "QTUM",
        "rel": "MYCOIN",
    })))
    .unwrap();
    assert!(orderbook.0.is_success(), "!orderbook: {}", rc.1);

    let orderbook: OrderbookResponse = json::from_str(&orderbook.1).unwrap();
    assert_eq!(orderbook.asks[0].coin, "QTUM");
    assert_eq!(orderbook.asks[0].address, segwit_addr);
    block_on(mm.stop()).unwrap();
}

#[test]
fn test_trade_qrc20() { trade_base_rel(("QICK", "QORTY")); }

#[test]
fn trade_test_with_maker_segwit() { trade_base_rel(("QTUM", "MYCOIN")); }

#[test]
fn trade_test_with_taker_segwit() { trade_base_rel(("MYCOIN", "QTUM")); }

#[test]
#[ignore]
fn test_trade_qrc20_utxo() { trade_base_rel(("QICK", "MYCOIN")); }

#[test]
#[ignore]
fn test_trade_utxo_qrc20() { trade_base_rel(("MYCOIN", "QICK")); }
