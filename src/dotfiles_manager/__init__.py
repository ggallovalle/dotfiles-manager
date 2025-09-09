from typing import Any
from ruamel.yaml import YAML
import ruamel.yaml.comments as c
import ruamel.yaml.nodes as n
from jinja2 import Environment, Template, pass_environment, pass_context
from jinja2.runtime import Context

class EvalStr:
    yaml_tag = u'!eval'

    jinja = Environment

    _path: list[n.Node]
    _node: n.ScalarNode
    _cached_str: str | None = None
    def __init__(self, path: list[n.Node], node: n.ScalarNode) -> None:
        self._path = path
        self._node = node

    @property
    def template(self) -> str:
        return self._node.value
    
    @template.setter
    def template(self, value: str) -> None:
        self._node.value = value

    @property
    def path(self) -> str:
        last = self._path[-1]
        if isinstance(last, n.ScalarNode):
            return last.value
        else:
            return ""

    @classmethod
    def from_yaml(cls, constructor, node: n.ScalarNode):
        history: list[n.Node] = list(constructor.constructed_objects) # 
        return cls(history, node)

    @classmethod
    def try_evaluate(cls, value: Any) -> Any:
        if isinstance(value, str) and cls.jinja is not None:
            template: Template = cls.jinja.from_string(value)
            return template.render()
        return value

    @classmethod
    def to_yaml(cls, representer, instance):
        # return instance._node
        node = instance._node
        value = cls.try_evaluate(instance.template)
        return representer.represent_scalar('tag:yaml.org,2002:str', value, node.style, node.anchor)
        # return representer.represent_scalar(cls.yaml_tag, instance.template)

    @classmethod
    def register_environment(cls, enviroment) -> None:
        cls.jinja = enviroment


    def __str__(self):
        if self._cached_str is None:
            self._cached_str = self.try_evaluate(self.template)
        return self._cached_str

def modify(path: str, value: c.CommentedMap) -> None:
    node = None
    for item in path.split('.'):
        if node is None:
            node = value.get(item)
        else:
            node = node.get(item)

    if isinstance(node, EvalStr):
    # target: EvalStr = value['dotfiles'][0]['target']
        node.template = node.template + ' modified_value'

# @pass_context
# def custom_finalize(context: Context, value: Any):
#     if isinstance(value, EvalStr):
#         # return "custom rendering of EvalStr"
#         path = value.path
#         resolved = context.environment.from_string(value.template).render(**context.parent)
#         return f"path({path})${ {resolved} }"
#     return value

# @pass_environment
# def custom_finalize(environment: Environment, value: Any):
#     if isinstance(value, EvalStr):
#         path = value.path
#         resolved = environment.from_string(value.template).render()
#         return f"path({path})${ {resolved} }"
#     return value

def make_jinja_enviroment() -> Environment:
    from jinja2 import Environment
    from jinja2 import DictLoader
    from jinja2 import select_autoescape
    from jinja2 import StrictUndefined
    from jinja2 import Template
    import os

    env = Environment(
        loader=DictLoader({}),
        autoescape=False,
        undefined=StrictUndefined,
        keep_trailing_newline=True,
        lstrip_blocks=True,
        trim_blocks=True,
        # finalize=custom_finalize,
    )
    env.filters['abspath'] = lambda x: os.path.abspath(os.path.expanduser(x))
    env.globals['env'] = lambda k, d=None: os.environ.get(k, d)
    return env

def jinja_main() -> None:
    env = make_jinja_enviroment()
    template = env.get_template('dotfiles.wsl.yaml')
    context = raumel_main()["properties"]
    rendered = template.render(context)
    print(rendered)

def raumel_main() -> None:
    typ="rt" # round-trip to preserve comments
    # typ="safe" # this doesn't output anything for some reason
    yaml = YAML(typ=typ)
    yaml.register_class(EvalStr)
    path = "dotfiles.wsl.yaml"
    source_val = yaml.load(open(path))
    # modify("properties.paths.config", source_val)
    # get a dict from a CommentedMap
    source_val_dict = dict(source_val)
    # print(source_val_dict)
    environment = make_jinja_enviroment()
    environment.globals.update(source_val_dict["properties"])
    EvalStr.register_environment(environment)
    yaml.dump(source_val, open("dotfiles_out.yaml", "w"))
    return source_val_dict

def main() -> None:
    # jinja_main()
    raumel_main()
    print("Hello from dotfiles-manager!")
