//! TEST DATA: exercise the real doc API and derived script output on native AND WASM.
use std::{cell::RefCell, rc::Rc};
use vellum_contracts::{
    fakes::{InMemoryDocStore, SequentialIds},
    samples,
};
use vellum_core::{
    doc_api::{self, ExecuteContext, ExecuteRequest, NoServices},
    guard::GuardState,
    ports::{DocStore, ScriptRuntime, Timestamp},
    script_nodes,
};
use vellum_types::Node;
pub fn transcript(runtime: &dyn ScriptRuntime) -> String {
    let before = samples::document();
    let mut store = InMemoryDocStore::new(&before, 1);
    let out=doc_api::execute(runtime,&mut store,ExecuteRequest {source:"const n=doc.insert('page00001',{type:'frame',width:100,height:100}); for(let i=0;i<5;i++)doc.insert(n,{type:'rect',width:Math.round(Math.random()*20)+1,height:10}); doc.copy(n,'page00001');return [doc.apiVersion,doc.query({type:'rect'}).length];",origin:vellum_types::Origin::new(samples::alice()),validate_only:false,limits:doc_api::EXECUTE_LIMITS},ExecuteContext {ids:Rc::new(RefCell::new(SequentialIds::new())),services:Rc::new(NoServices),guard:&mut GuardState::default(),now:Timestamp(0)}).unwrap();
    let mut text = format!(
        "doc-api:{:?}:{}\n",
        out.value,
        serde_json::to_string(&store.snapshot()).unwrap()
    );
    store.undo(&samples::alice()).unwrap();
    assert_eq!(store.snapshot(), before);
    let script:Node=serde_json::from_value(serde_json::json!({"type":"script","id":"script","name":"TEST DATA","src":"chart.js","width":100,"height":100})).unwrap();
    let output=script_nodes::evaluate(runtime,&before,&script,"// @schema 1\n// @input count: number(min=1) = 3\nreturn Array.from({length:doc.inputs.count},()=>({type:'rect',width:Math.round(Math.random()*20)+1,height:10}));",&vellum_core::resolver::variables::Theme::defaults(&before),&mut SequentialIds::new()).unwrap();
    text.push_str(&format!(
        "script-nodes:{}\n",
        serde_json::to_string(&output.nodes).unwrap()
    ));
    text
}
